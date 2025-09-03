# temp-pair-enocean

How hot is it outside? How hot is it inside? Does it make sense to open the window?

This is a design based on a handful of [MikroElektronika](https://www.mikroe.com/) components that
reads temperature values from EnOcean sensors and displays them on 7-segment displays. As an
additional bonus, it varies the brightness of the digits according to the ambient brightness.

## Components

| component                 | product number | placement           | usage |
| ------------------------- | -------------- | ------------------- | ----- |
| Clicker 4 for STM32F745VG | MIKROE-6331    | base                | the board with the microcontroller |
| EnOcean 3 Click           | MIKROE-3653    | mikroBUS slot 1     | RF interface to EnOcean sensors |
| Flash 10 Click            | MIKROE-5289    | mikroBUS slot 2     | storage of EnOcean addresses |
| I2C SPI Shuttle board     | see Note 1     | mikroBUS slot 3     | isolation of the temperature display |
| UT-S 7-SEG B 2 Click      | MIKROE-5912    | shuttle on slot 3   | temperature display |
| Shuttle Click             | MIKROE-2880    | mikroBUS slot 4     | port expander |
| 8800 Retro Click          | MIKROE-4771    | shuttle 1 on slot 4 | human-machine interface |
| Ambient 24 Click          | MIKROE-6534    | shuttle 2 on slot 4 | ambient light sensor |

Note 1: The TLC5947 on the UT-S 7-SEG B 2 Click doesn't have a chip-select pin, so it cannot coexist
with other devices on a SPI bus. It has therefore been placed on its own SPI bus controlled via I2C
using an [I2C SPI Shuttle
board](https://github.com/RavuAlHemio/mikrobus-boards/tree/main/i2c-spi-shuttle-mikrobus-board)
which I hope actually works because I designed it myself...

## mikroBUS pins

| slot | mikroBUS port | µC pin | usage |
| ----:| ------------- | ------ | ----- |
|    1 | RST           | PC15   | reset EnOcean module |
|    1 | UART M→B      | PA3    | UART EnOcean module to board |
|    1 | UART B→M      | PA2    | UART board to EnOcean module |
|    2 | RST           | PE7    | reset flash chip |
|    2 | SPI CS        | PE8    | SPI: select flash chip |
|    2 | SPI SCK       | PA5 ⁎  | SPI clock, slots 1-3 (here: flash) |
|    2 | SPI CIPO      | PA6 ⁎  | SPI peripheral → controller, slots 1-3 (here: flash) |
|    2 | SPI COPI      | PA7 ⁎  | SPI controller → peripheral, slots 1-3 (here: flash) |
|    2 | PWM           | PD12   | flash write protection |
|    3 | RST           | PD11   | reset I2C-SPI bridge |
|    3 | I2C SCL       | PB10 ⁎ | I2C clock, slots 1-4 (here: SPI bridge to 7seg) |
|    3 | I2C SDA       | PB11 ⁎ | I2C data, slots 1-4 (here: SPI bridge to 7seg) |
|  4.1 | INT           | PB14   | button pressed interrupt |
|  4.1 | I2C SCL       | PB10 ⁎ | I2C clock, slots 1-4 (here: buttons & LEDs) |
|  4.1 | I2C SDA       | PB11 ⁎ | I2C data, slots 1-4 (here: buttons & LEDs) |
|  4.2 | SPI CS        | PD15   | Click ID for light sensor board |
|  4.2 | I2C SCL       | PB10 ⁎ | I2C clock, slots 1-4 (here: light sensor) |
|  4.2 | I2C SDA       | PB11 ⁎ | I2C data, slots 1-4 (here: light sensor) |
|  4.3 | UART M→B      | PD9    | reserved for emergency UART, PC to board |
|  4.3 | UART B→M      | PD8    | reserved for emergency UART, board to PC |

⁎ This pin is used by multiple mikroBUS boards cooperatively.

## µC pin configuration

| pin  | mode    | description |
| ---- | ------- | ----------- |
| PA2  | AF7 PP? | USART2 Tx (board to EnOcean module) |
| PA3  | AF7 PP? | USART2 Rx (EnOcean module to board) |
| PA5  | AF5 PP? | SPI1 SCK (flash) |
| PA6  | AF5 PP? | SPI1 CIPO (flash) |
| PA7  | AF5 PP? | SPI1 COPI (flash) |
| PB10 | AF4 PP? | I2C2 SCL (7seg SPI bridge / buttons & LEDs / light sensor) |
| PB11 | AF4 PP? | I2C2 SDA (7seg SPI bridge / buttons & LEDs / light sensor) |
| PB14 | DI PU   | button pushed interrupt (`SYSCFG.EXTICR4.EXTI14 = PB`, `EXTI.IMR.IM14 = false`) |
| PC15 | DO PP   | reset EnOcean module |
| PD8  | AF7 PP? | USART3 Tx (emergency, board to PC) |
| PD9  | AF7 PP? | USART3 Rx (emergency, PC to board) |
| PD11 | DO PP   | 7seg SPI bridge reset |
| PD12 | DO PP   | flash write protection |
| PD15 | DI Flt  | not used |
| PE7  | DO PP   | reset flash chip |
| PE8  | DO PP   | flash chip select for SPI1 |

## I2C addresses

| address   | mikroBUS port | peripheral         | role |
| --------- | ------------- | ------------------ | ---- |
| 0b0000000 | 4 s1          | AMS AS1115         | buttons & LEDs |
| 0b0101001 | 4 s2          | Vishay VEML4031X00 | light sensor |
| 0b01010ab | 3             | NXP SC18IS606PW    | I2C-SPI bridge |
| 0b11100ab | 3             | NXP PCA9538D       | I2C port expander |

(Make sure to set the switches on the I2C SPI shuttle board so that the addresses don't conflict
with the lgiht sensor.)
