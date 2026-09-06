#![no_std]
#![no_main]

use esp_hal::{
    clock::ClockControl,
    gpio::{IO, Input, Output, PullDown},
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

const CS: u32 = 20;
const DC: u32 = 21;
const RST: u32 = 7;
const SCL: u32 = 8;
const SDA: u32 = 10;

const shft: u32 = 2;
const hr: u32 = 3;
const min: u32 = 4;
const tone: u32 = 5;
const mult: u32 = 6;
const buz: u32 = 9;

#[derive(PartialEq, Clone, Copy)]
enum Mode {
    Off,
    armed,
    ringing,
    snooze,
}

#[derive(PartialEq, Clone, Copy)]
enum Dir {
    up,
    down,
}

#[derive(PartialEq, Clone, Copy)]
enum Tone {
    cont,
    beep,
    fast,
    siren,
    melody,
}

#[derive(PartialEq, Clone, Copy)]
enum Step {
    _1x = 1,
    _2x = 2,
    _5x = 5,
    _10x = 10,
    _12x = 12,
}

impl Step {
    fn next(&self) -> Self {
        match self {
            Step::_1x => Step::_2x,
            Step::_2x => Step::_5x,
            Step::_5x => Step::_10x,
            Step::_10x => Step::_12x,
            Step::_12x => Step::_1x,
        }
    }
}

impl Tone {
    fn next(&self) -> Self {
        match self {
            Tone::cont => Tone::beep,
            Tone::beep => Tone::fast,
            Tone::fast => Tone::siren,
            Tone::siren => Tone::melody,
            Tone::melody => Tone::cont,
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
            dir: Dir::up,
            alarm_h: 7,
            alarm_m: 0,
            now_h: 12,
            now_m: 0,
            now_s: 0,
            tone: Tone::beep,
            step: Step::_1x,
            snooze_m: 5,
            snooze_s: 0,
            phase: 0,
        }
    }
}

struct Buttons {
    shft: Input<PullDown>,
    hr: Input<PullDown>,
    min: Input<PullDown>,
    tone: Input<PullDown>,
    mult: Input<PullDown>,
}

impl Buttons {
    fn read(&self) -> ButtonState {
        ButtonState {
            shft: self.shft.is_high(),
            hr: self.hr.is_high(),
            min: self.min.is_high(),
            tone: self.tone.is_high(),
            mult: self.mult.is_high(),
        }
    }
}

struct ButtonState {
    shft: bool,
    hr: bool,
    min: bool,
    tone: bool,
    mult: bool,
}

struct Debounce {
    shft: u8,
    hr: u8,
    min: u8,
    tone: u8,
    mult: u8,
}

impl Debounce {
    fn new() -> Self {
        Self { shft: 0, hr: 0, min: 0, tone: 0, mult: 0 }
    }

    fn update(&mut self, state: &ButtonState) -> ProcessedButtons {
        let shft = self.process(&mut self.shft, state.shft);
        let hr = self.process(&mut self.hr, state.hr);
        let min = self.process(&mut self.min, state.min);
        let tone = self.process(&mut self.tone, state.tone);
        let mult = self.process(&mut self.mult, state.mult);
        ProcessedButtons { shft, hr, min, tone, mult }
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
    shft: bool,
    hr: bool,
    min: bool,
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

    let sclk = io.pins.gpio8.into_push_pull_output();
    let mosi = io.pins.gpio10.into_push_pull_output();

    let spi = Spi::new(p.SPI2, 40u32.MHz(), SpiMode::Mode0, &clk)
        .with_pins(Some(sclk), Some(mosi), None, None);

    let cs = io.pins.gpio20.into_push_pull_output();
    let dc = io.pins.gpio21.into_push_pull_output();
    let rst = io.pins.gpio7.into_push_pull_output();
    let iface = SPIInterface::new(spi, dc, cs);
    let mut disp = ST7789::new(iface, rst, 76, 284);
    disp.init(&mut delay).unwrap();

    let mut buttons = Buttons {
        shft: io.pins.gpio2.into_pull_down_input(),
        hr: io.pins.gpio3.into_pull_down_input(),
        min: io.pins.gpio4.into_pull_down_input(),
        tone: io.pins.gpio5.into_pull_down_input(),
        mult: io.pins.gpio6.into_pull_down_input(),
    };
    let mut buz = io.pins.gpio9.into_push_pull_output();

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

        if st.mode == Mode::snooze && second_tick {
            st.snooze_s += 1;
            if st.snooze_s >= (st.snooze_m as u32 * 60) {
                st.mode = Mode::ringing;
                st.snooze_s = 0;
                st.phase = 0;
                draw(&mut disp, &st);
            }
        }

        let pressed = debounce.update(&buttons.read());

        if pressed.shft {
            handle_shift(&mut st);
            buz.set_1();
            delay.delay_ms(50u32);
            buz.set_0();
            draw(&mut disp, &st);
        }

        if pressed.hr {
            handle_hour(&mut st);
            buz.set_1();
            delay.delay_ms(50u32);
            buz.set_0();
            draw(&mut disp, &st);
        }

        if pressed.min {
            handle_minute(&mut st);
            buz.set_1();
            delay.delay_ms(50u32);
            buz.set_0();
            draw(&mut disp, &st);
        }

        if pressed.tone {
            handle_tone(&mut st);
            buz.set_1();
            delay.delay_ms(50u32);
            buz.set_0();
            draw(&mut disp, &st);
        }

        if pressed.mult {
            handle_mult(&mut st);
            buz.set_1();
            delay.delay_ms(50u32);
            buz.set_0();
            draw(&mut disp, &st);
        }

        if st.mode == Mode::ringing {
            play_tone(&mut buz, &mut st);
        } else {
            buz.set_0();
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
    if st.mode == Mode::armed && st.now_h == st.alarm_h && st.now_m == st.alarm_m && st.now_s == 0 {
        st.mode = Mode::ringing;
        st.phase = 0;
    }
}

fn handle_shift(st: &mut State) {
    if st.mode != Mode::ringing {
        st.dir = match st.dir {
            Dir::up => Dir::down,
            Dir::down => Dir::up,
        };
    }
}

fn handle_hour(st: &mut State) {
    if st.mode == Mode::ringing {
        start_snooze(st);
    } else if st.mode == Mode::Off || st.mode == Mode::armed {
        let delta = st.step as u8;
        match st.dir {
            Dir::up => st.alarm_h = (st.alarm_h + delta) % 24,
            Dir::down => st.alarm_h = (st.alarm_h + 24 - delta) % 24,
        }
        st.mode = Mode::armed;
    }
}

fn handle_minute(st: &mut State) {
    if st.mode == Mode::ringing {
        start_snooze(st);
    } else if st.mode == Mode::Off || st.mode == Mode::armed {
        let delta = st.step as u8;
        match st.dir {
            Dir::up => st.alarm_m = (st.alarm_m + delta) % 60,
            Dir::down => st.alarm_m = (st.alarm_m + 60 - delta) % 60,
        }
        st.mode = Mode::armed;
    }
}

fn handle_tone(st: &mut State) {
    if st.mode == Mode::ringing {
        start_snooze(st);
    } else {
        st.tone = st.tone.next();
    }
}

fn handle_mult(st: &mut State) {
    if st.mode == Mode::ringing {
        st.mode = Mode::Off;
        st.snooze_s = 0;
        st.phase = 0;
    } else {
        st.step = st.step.next();
    }
}

fn start_snooze(st: &mut State) {
    st.mode = Mode::snooze;
    st.snooze_m = st.step as u8;
    st.snooze_s = 0;
    st.phase = 0;
}

fn play_tone(buz: &mut Output, st: &mut State) {
    st.phase += 1;
    match st.tone {
        Tone::cont => buz.set_1(),
        Tone::beep => {
            if (st.phase / 50) % 2 == 0 { buz.set_1() } else { buz.set_0() }
        }
        Tone::fast => {
            if (st.phase / 20) % 2 == 0 { buz.set_1() } else { buz.set_0() }
        }
        Tone::siren => {
            let period = 40 + (st.phase / 100) % 40;
            if (st.phase / period) % 2 == 0 { buz.set_1() } else { buz.set_0() }
        }
        Tone::melody => {
            let pattern = [200, 150, 250, 150, 200, 150, 250, 150];
            let idx = (st.phase / 25) % 8;
            if st.phase % 25 < pattern[idx] / 10 { buz.set_1() } else { buz.set_0() }
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
        Mode::armed => "ARM",
        Mode::ringing => "RING",
        Mode::snooze => "SNZ",
    };
    let dir = match st.dir {
        Dir::up => "+",
        Dir::down => "-",
    };
    let step = match st.step {
        Step::_1x => "1x",
        Step::_2x => "2x",
        Step::_5x => "5x",
        Step::_10x => "10x",
        Step::_12x => "12x",
    };
    let tone = match st.tone {
        Tone::cont => "CONT",
        Tone::beep => "BEEP",
        Tone::fast => "FAST",
        Tone::siren => "SIRN",
        Tone::melody => "MELD",
    };
    let _ = write!(&mut buf, "{} {} {} {}", status, dir, step, tone);
    let _ = Text::new(&buf, embedded_graphics::geometry::Point::new(10, 60), style).draw(disp);

    if st.mode == Mode::snooze {
        buf.clear();
        let remain = (st.snooze_m as u32 * 60 - st.snooze_s) / 60;
        let _ = write!(&mut buf, "SNZ {}m", remain);
        let _ = Text::new(&buf, embedded_graphics::geometry::Point::new(10, 80), style).draw(disp);
    }
}