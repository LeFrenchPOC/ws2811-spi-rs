//! This prerenders the data, so that no calculations have to be performed while sending the data.
//!
//! This approach minimizes timing issues, at the cost of much higher ram usage.
//! It also increases the needed time.

use embedded_hal as hal;

use hal::spi::{FullDuplex, Mode, Phase, Polarity};

use core::marker::PhantomData;

use smart_leds_trait::{SmartLedsWrite, RGB8, RGBW};

use nb;
use nb::block;

/// SPI mode that can be used for this crate
///
/// Provided for convenience
/// Doesn't really matter
pub const MODE: Mode = Mode {
    polarity: Polarity::IdleLow,
    phase: Phase::CaptureOnFirstTransition,
};

#[derive(Debug)]
pub enum Error<E> {
    OutOfBounds,
    Spi(E),
}

pub mod devices {
    pub struct Ws2811;
}

pub struct Ws2811<'a, SPI, DEVICE = devices::Ws2811> {
    spi: SPI,
    data: &'a mut [u8],
    index: usize,
    device: PhantomData<DEVICE>,
}

impl<'a, SPI, E> Ws2811<'a, SPI>
where
    SPI: FullDuplex<u8, Error = E>,
{
    /// Use Ws2811 devices via spi
    ///
    /// The SPI bus should run within 2 MHz to 3.8 MHz
    ///
    /// You may need to look at the datasheet and your own hal to verify this.
    ///
    /// You need to provide a buffer `data`, whoose length is at least 12 * the
    /// length of the led strip + 20 byes (or 40, if using the `mosi_idle_high` feature)
    ///
    /// Please ensure that the mcu is pretty fast, otherwise weird timing
    /// issues will occur
    pub fn new(spi: SPI, data: &'a mut [u8]) -> Self {
        Self {
            spi,
            data,
            index: 0,
            device: PhantomData {},
        }
    }
}

impl<'a, SPI, D, E> Ws2811<'a, SPI, D>
where
    SPI: FullDuplex<u8, Error = E>,
{
    /// Write a single byte for Ws2811 devices
    fn write_byte(&mut self, mut data: u8) -> Result<(), Error<E>> {
        // Send two bits in one spi byte. High time first, then the low time
        // The maximum for T0H is 500ns, the minimum for one bit 1063 ns.
        // These result in the upper and lower spi frequency limits
        let patterns = [0b1000_1000, 0b1000_1110, 0b11101000, 0b11101110];

        if self.index > self.data.len() - 4 {
            return Err(Error::OutOfBounds);
        }
        for _ in 0..4 {
            let bits = (data & 0b1100_0000) >> 6;
            self.data[self.index] = patterns[bits as usize];
            self.index += 1;
            data <<= 2;
        }
        Ok(())
    }

    fn send_data(&mut self) -> Result<(), E> {
        // We introduce an offset in the fifo here, so there's always one byte in transit
        // Some MCUs (like the stm32f1) only a one byte fifo, which would result
        // in overrun error if two bytes need to be stored
        block!(self.spi.send(0))?;
        if cfg!(feature = "mosi_idle_high") {
            for _ in 0..140 {
                block!(self.spi.send(0))?;
                block!(self.spi.read())?;
            }
        }
        for b in self.data[..self.index].iter() {
            block!(self.spi.send(*b))?;
            block!(self.spi.read())?;
        }
        for _ in 0..140 {
            block!(self.spi.send(0))?;
            block!(self.spi.read())?;
        }
        // Now, resolve the offset we introduced at the beginning
        block!(self.spi.read())?;
        Ok(())
    }
}

impl<'a, SPI, E> SmartLedsWrite for Ws2811<'a, SPI>
where
    SPI: FullDuplex<u8, Error = E>,
{
    type Error = Error<E>;
    type Color = RGB8;
    /// Write all the items of an iterator to a Ws2811 strip
    fn write<T, I>(&mut self, iterator: T) -> Result<(), Error<E>>
    where
        T: Iterator<Item = I>,
        I: Into<Self::Color>,
    {
        self.index = 0;

        for item in iterator {
            let item = item.into();
            self.write_byte(item.r)?;
            self.write_byte(item.b)?;
            self.write_byte(item.g)?;
        }
        self.send_data().map_err(|e| Error::Spi(e))
    }
}
