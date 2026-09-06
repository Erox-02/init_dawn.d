#![no_std]
#![no_main]

use esp_hal::{
    clock::ClockControl,
    gpio::IO,
    peripherals::Peripherals,
    prelude::*,
    spi::{master::Spi, SpiMode},
    system::SystemControl,
    delay::Delay,
};
use esp_backtrace as _;
use st7789::ST7789;
use display_interface_spi::SPIInterface;
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    text::Text,
};

const CS: u32 = 0;
const DC: u32 = 7;
const RST: u32 = 9;
const SCL: u32 = 10;
const SDA: u32 = 8;

const SHIFT: u32 = 1;
const HOUR: u32 = 2;
const MINUTE: u32 = 3;
const TONE: u32 = 4;
const MULT: u32 = 5;
const BUZZ: u32 = 6;

#[derive(PartialEq, Clone, Copy)]
enum Mode {
    Off,
    Armed,
    Ringing,
    Snooze,
}

#[derive(PartialEq, Clone, Copy)]
enum Dir {
    Up,
    Down,
}

#[derive(PartialEq, Clone, Copy)]
enum Tone {
    Continuous,
    Beep,
    Fast,
    Siren,
    Melody,
}

#[derive(PartialEq, Clone, Copy)]
enum Step {
    One = 1,
    Two = 2,
    Five = 5,
    Ten = 10,
    Twelve = 12,
}

impl Step {
    fn next(&self) -> Self {
        match self {
            Step::One => Step::Two,
            Step::Two => Step::Five,
            Step::Five => Step::Ten,
            Step::Ten => Step::Twelve,
            Step::Twelve => Step::One,
        }
    }
}

impl Tone {
    fn next(&self) -> Self {
        match self {
            Tone::Continuous => Tone::Beep,
            Tone::Beep => Tone::Fast,
            Tone::Fast => Tone::Siren,
            Tone::Siren => Tone::Melody,
            Tone::Melody => Tone::Continuous,
        }
    }
}

struct State {
    mode: Mode,
    dir: Dir,
    alarm_h: u8,
    alarm_m: u8,
    now_h: u8,
    now_m: u8,
    now_s: u8,
    tone: Tone,
    step: Step,
    snooze_m: u8,
    snooze_s: u32,
    phase: u32,
}

impl State {
    fn new() -> Self {
        Self {
            mode: Mode::Off,
            dir: Dir::Up,
            alarm_h: 7,
            alarm_m: 0,
            now_h: 12,
            now_m: 0,
            now_s: 0,
            tone: Tone::Beep,
            step: Step::One,
            snooze_m: 5,
            snooze_s: 0,
            phase: 0,
        }
    }
}

struct Buttons {
    shift: Input,
    hour: Input,
    minute: Input,
    tone: Input,
    mult: Input,
}

impl Buttons {
    fn read(&mut self) -> ButtonState {
        ButtonState {
            shift: self.shift.is_high(),
            hour: self.hour.is_high(),
            minute: self.minute.is_high(),
            tone: self.tone.is_high(),
            mult: self.mult.is_high(),
        }
    }
}

struct ButtonState {
    shift: bool,
    hour: bool,
    minute: bool,
    tone: bool,
    mult: bool,
}

struct Debounce {
    shift: u8,
    hour: u8,
    minute: u8,
    tone: u8,
    mult: u8,
}

impl Debounce {
    fn new() -> Self {
        Self { shift: 0, hour: 0, minute: 0, tone: 0, mult: 0 }
    }

    fn update(&mut self, state: &ButtonState) -> ProcessedButtons {
        let shift = self.process(&mut self.shift, state.shift);
        let hour = self.process(&mut self.hour, state.hour);
        let minute = self.process(&mut self.minute, state.minute);
        let tone = self.process(&mut self.tone, state.tone);
        let mult = self.process(&mut self.mult, state.mult);
        ProcessedButtons { shift, hour, minute, tone, mult }
    }

    fn process(&mut self, counter: &mut u8, pressed: bool) -> bool {
        if pressed {
            if *counter < 5 {
                *counter += 1;
                *counter == 5
            } else {
                false
            }
        } else {
            *counter = 0;
            false
        }
    }
}

struct ProcessedButtons {
    shift: bool,
    hour: bool,
    minute: bool,
    tone: bool,
    mult: bool,
}

#[entry]
fn main() -> ! {
    let p = Peripherals::take().unwrap();
    let sys = SystemControl::new(p.SYSTEM);
    let clk = ClockControl::boot_defaults(sys.clock_control).freeze();
    let mut delay = Delay::new(&clk);
    let io = IO::new(p.GPIO, p.IO_MUX);

    let sclk = io.pins.gpio10.into_push_pull_output();
    let mosi = io.pins.gpio8.into_push_pull_output();
    let miso = io.pins.gpio9.into_push_pull_output();

    let spi = Spi::new(p.SPI2, 40u32.MHz(), SpiMode::Mode0, &clk)
        .with_pins(Some(sclk), Some(mosi), Some(miso), None);

    let cs = io.pins.gpio0.into_push_pull_output();
    let dc = io.pins.gpio7.into_push_pull_output();
    let rst = io.pins.gpio9.into_push_pull_output();
    let iface = SPIInterface::new(spi, dc, cs);
    let mut disp = ST7789::new(iface, rst, 76, 284);
    disp.init(&mut delay).unwrap();

    let mut buttons = Buttons {
        shift: io.pins.gpio1.into_pull_down_input(),
        hour: io.pins.gpio2.into_pull_down_input(),
        minute: io.pins.gpio3.into_pull_down_input(),
        tone: io.pins.gpio4.into_pull_down_input(),
        mult: io.pins.gpio5.into_pull_down_input(),
    };
    let mut buz = io.pins.gpio6.into_push_pull_output();

    let mut st = State::new();
    let mut debounce = Debounce::new();
    let mut tick = 0u32;

    draw(&mut disp, &st);

    loop {
        delay.delay_ms(10u32);
        tick += 1;

        let second_tick = tick >= 100;
        if second_tick {
            tick = 0;
            update_clock(&mut st);
            check_alarm(&mut st);
            draw(&mut disp, &st);
        }

        if st.mode == Mode::Snooze && second_tick {
            st.snooze_s += 1;
            if st.snooze_s >= (st.snooze_m as u32 * 60) {
                st.mode = Mode::Ringing;
                st.snooze_s = 0;
                st.phase = 0;
                draw(&mut disp, &st);
            }
        }

        let pressed = debounce.update(&buttons.read());

        if pressed.shift {
            handle_shift(&mut st);
            buz.set_high();
            delay.delay_ms(50u32);
            buz.set_low();
            draw(&mut disp, &st);
        }

        if pressed.hour {
            handle_hour(&mut st);
            buz.set_h            handle_minute(&mut st);
igh();
            delay.delay_ms(50u32);
            buz.set_low();
            draw(&mut disp, &st);
        }

        if pressed.minute {
            buz.set_high();
            delay.delay_ms(50u32);
            buz.set_low();
            draw(&mut disp, &st);
        }

        if pressed.tone {
            handle_tone(&mut st);
            buz.set_high();
            delay.delay_ms(50u32);
            buz.set_low();
            draw(&mut disp, &st);
        }

        if pressed.mult {
            handle_mult(&mut st);
            buz.set_high();
            delay.delay_ms(50u32);
            buz.set_low();
            draw(&mut disp, &st);
        }

        if st.mode == Mode::Ringing {
            play_tone(&mut buz, &mut st);
        } else {
            buz.set_low();
        }
    }
}

fn update_clock(st: &mut State) {
    st.now_s += 1;
    if st.now_s >= 60 {
        st.now_s = 0;
        st.now_m += 1;
        if st.now_m >= 60 {
            st.now_m = 0;
            st.now_h = (st.now_h + 1) % 24;
        }
    }
}

fn check_alarm(st: &mut State) {
    if st.mode == Mode::Armed && st.now_h == st.alarm_h && st.now_m == st.alarm_m && st.now_s == 0 {
        st.mode = Mode::Ringing;
        st.phase = 0;
    }
}

fn handle_shift(st: &mut State) {
    if st.mode != Mode::Ringing {
        st.dir = match st.dir {
            Dir::Up => Dir::Down,
            Dir::Down => Dir::Up,
        };
    }
}

fn handle_hour(st: &mut State) {
    if st.mode == Mode::Ringing {
        start_snooze(st);
    } else if st.mode == Mode::Off || st.mode == Mode::Armed {
        let delta = st.step as u8;
        match st.dir {
            Dir::Up => st.alarm_h = (st.alarm_h + delta) % 24,
            Dir::Down => st.alarm_h = (st.alarm_h + 24 - delta) % 24,
        }
        st.mode = Mode::Armed;
    }
}

fn handle_minute(st: &mut State) {
    if st.mode == Mode::Ringing {
        start_snooze(st);
    } else if st.mode == Mode::Off || st.mode == Mode::Armed {
        let delta = st.step as u8;
        match st.dir {
            Dir::Up => st.alarm_m = (st.alarm_m + delta) % 60,
            Dir::Down => st.alarm_m = (st.alarm_m + 60 - delta) % 60,
        }
        st.mode = Mode::Armed;
    }
}

fn handle_tone(st: &mut State) {
    if st.mode == Mode::Ringing {
        start_snooze(st);
    } else {
        st.tone = st.tone.next();
    }
}

fn handle_mult(st: &mut State) {
    if st.mode == Mode::Ringing {
        st.mode = Mode::Off;
        st.snooze_s = 0;
        st.phase = 0;
    } else {
        st.step = st.step.next();
    }
}

fn start_snooze(st: &mut State) {
    st.mode = Mode::Snooze;
    st.snooze_m = st.step as u8;
    st.snooze_s = 0;
    st.phase = 0;
}

fn play_tone(buz: &mut Output, st: &mut State) {
    st.phase += 1;
    match st.tone {
        Tone::Continuous => buz.set_high(),
        Tone::Beep => {
            if (st.phase / 50) % 2 == 0 { buz.set_high() } else { buz.set_low() }
        }
        Tone::Fast => {
            if (st.phase / 20) % 2 == 0 { buz.set_high() } else { buz.set_low() }
        }
        Tone::Siren => {
            let period = 40 + (st.phase / 100) % 40;
            if (st.phase / period) % 2 == 0 { buz.set_high() } else { buz.set_low() }
        }
        Tone::Melody => {
            let pattern = [200, 150, 250, 150, 200, 150, 250, 150];
            let idx = (st.phase / 25) % 8;
            if st.phase % 25 < pattern[idx] / 10 { buz.set_high() } else { buz.set_low() }
        }
    }
}

fn draw(disp: &mut ST7789<SPIInterface<impl SpiDevice>>, st: &State) {
    disp.clear(st7789::Color::BLACK).unwrap();

    let style = MonoTextStyle::new(&FONT_6X10, st7789::Color::WHITE);
    let mut buf = heapless::String::<64>::new();

    use core::fmt::Write;
    let _ = write!(&mut buf, "{:02}:{:02}:{:02}", st.now_h, st.now_m, st.now_s);
    let _ = Text::new(&buf, embedded_graphics::geometry::Point::new(10, 20), style).draw(disp);

    buf.clear();
    let _ = write!(&mut buf, "AL {:02}:{:02}", st.alarm_h, st.alarm_m);
    let _ = Text::new(&buf, embedded_graphics::geometry::Point::new(10, 40), style).draw(disp);

    buf.clear();
    let status = match st.mode {
        Mode::Off => "OFF",
        Mode::Armed => "ARM",
        Mode::Ringing => "RING",
        Mode::Snooze => "SNZ",
    };
    let dir = match st.dir {
        Dir::Up => "+",
        Dir::Down => "-",
    };
    let step = match st.step {
        Step::One => "1x",
        Step::Two => "2x",
        Step::Five => "5x",
        Step::Ten => "10x",
        Step::Twelve => "12x",
    };
    let tone = match st.tone {
        Tone::Continuous => "CONT",
        Tone::Beep => "BEEP",
        Tone::Fast => "FAST",
        Tone::Siren => "SIRN",
        Tone::Melody => "MELD",
    };
    let _ = write!(&mut buf, "{} {} {} {}", status, dir, step, tone);
    let _ = Text::new(&buf, embedded_graphics::geometry::Point::new(10, 60), style).draw(disp);

    if st.mode == Mode::Snooze {
        buf.clear();
        let remain = (st.snooze_m as u32 * 60 - st.snooze_s) / 60;
        let _ = write!(&mut buf, "SNZ {}m", remain);
        let _ = Text::new(&buf, embedded_graphics::geometry::Point::new(10, 80), style).draw(disp);
    }
}