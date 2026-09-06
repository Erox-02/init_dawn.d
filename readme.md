[![Hack Club Stardance](https://img.shields.io/badge/Hack%20Club-Stardance-blueviolet)](https://stardance.hackclub.com/)

# init_dawn.d

## comps

- Seeed Studio XIAO ESP32-C3
- 1x 2.25in TFT Screen
- 1x 3.3V Piezo Buzzer
- 5x MX-Style Keyboard Switches

## wiring

gnd(pin 1) - gnd
vcc(pin 2) - 3.3v out
scl(pin 3) - D10
sda(pin 4) - D8
rst(pin 5) -D9
dc(pin 6) - D7
cs(pin 7) - D0
bl (pin 8) - 3.3v out

SW1-sw5 1 pin on 3.3v and other pin on D1-D5 
Buzzer on D6 and gnd 

## feature

- Set alarm time (hour/minute)
- Multiple step sizes (1x, 2x, 5x, 10x, 12x)
- 5 alarm tones (Continuous, Beep, Fast, Siren, Melody)
- Snooze function
- Direction switching (up/down)

## Controls


shift ->  change ladout (up/down)
hour  ->  adjust hours / snooze 
minute -> adjust minutes / snooze 
tone   -> change alarm tone / snooze 
multiplier -> change multipler / stop alarm 

## License

MIT

## author

> Dipanjan Dutta
> I use arch btw