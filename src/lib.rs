//! # Use ws2811 leds via spi
//!
//! - For usage with `smart-leds`
//! - Implements the `SmartLedsWrite` trait
//!
//! Needs a type implementing the `spi::FullDuplex` trait.
//!
//! The spi peripheral should run at 2MHz to 3.8 MHz

#![no_std]

use embedded_hal as hal;

pub mod prerendered;

use hal::spi::{FullDuplex, Mode, Phase, Polarity};

use core::marker::PhantomData;

use smart_leds_trait::{SmartLedsWrite, RGB8};

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

pub mod devices {
    pub struct Ws2811Rgb;
    pub struct Ws2811Rbg;
}

pub struct Ws2811<SPI, DEVICE = devices::Ws2811Rgb> {
    spi: SPI,
    device: PhantomData<DEVICE>,
}

impl<SPI, E> Ws2811<SPI>
where
    SPI: FullDuplex<u8, Error = E>,
{
    /// Use Ws2811 devices via spi RGB
    ///
    /// The SPI bus should run within 1.6 MHz to 3.2 MHz
    ///
    /// You may need to look at the datasheet and your own hal to verify this.
    ///
    /// Please ensure that the mcu is pretty fast, otherwise weird timing
    /// issues will occur
    pub fn new_rgb(spi: SPI) -> Self {
        Self {
            spi,
            device: PhantomData {},
        }
    }
}

impl<SPI, E> Ws2811<SPI, devices::Ws2811Rbg>
where
    SPI: FullDuplex<u8, Error = E>,
{
    /// Use Ws2811 devices via spi RBG
    ///
    /// The SPI bus should run within 1.6 MHz to 3.2 MHz
    ///
    /// You may need to look at the datasheet and your own hal to verify this.
    ///
    /// Please ensure that the mcu is pretty fast, otherwise weird timing
    /// issues will occur
    pub fn new_rbg(spi: SPI) -> Self {
        Self {
            spi,
            device: PhantomData {},
        }
    }
}

impl<SPI, D, E> Ws2811<SPI, D>
where
    SPI: FullDuplex<u8, Error = E>,
{
    /// Write a single byte for Ws2811 devices
    fn write_byte(&mut self, mut data: u8) -> Result<(), E> {
        // Send two bits in one spi byte. High time first, then the low time
        // The maximum for T0H is 500ns, the minimum for one bit 1063 ns.
        // These result in the upper and lower spi frequency limits
        let patterns = [0b1000_1000, 0b1000_1110, 0b11101000, 0b11101110];
        for _ in 0..4 {
            let bits = (data & 0b1100_0000) >> 6;
            block!(self.spi.send(patterns[bits as usize]))?;
            block!(self.spi.read()).ok();
            data <<= 2;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), E> {
        // Should be > 300μs, so for an SPI Freq. of 3.8MHz, we have to send at least 1140 low bits or 140 low bytes
        for _ in 0..140 {
            block!(self.spi.send(0))?;
            block!(self.spi.read()).ok();
        }
        Ok(())
    }
}

impl<SPI, E> SmartLedsWrite for Ws2811<SPI>
where
    SPI: FullDuplex<u8, Error = E>,
{
    type Error = E;
    type Color = RGB8;
    /// Write all the items of an iterator to a Ws2811 strip
    fn write<T, I>(&mut self, iterator: T) -> Result<(), E>
    where
        T: Iterator<Item = I>,
        I: Into<Self::Color>,
    {
        // We introduce an offset in the fifo here, so there's always one byte in transit
        // Some MCUs (like the stm32f1) only a one byte fifo, which would result
        // in overrun error if two bytes need to be stored
        block!(self.spi.send(0))?;
        if cfg!(feature = "mosi_idle_high") {
            self.flush()?;
        }

        for item in iterator {
            let item = item.into();
            self.write_byte(item.r)?;
            self.write_byte(item.g)?;
            self.write_byte(item.b)?;
        }
        self.flush()?;
        // Now, resolve the offset we introduced at the beginning
        block!(self.spi.read())?;
        Ok(())
    }
}

impl<SPI, E> SmartLedsWrite for Ws2811<SPI, devices::Ws2811Rbg>
where
    SPI: FullDuplex<u8, Error = E>,
{
    type Error = E;
    type Color = RGB8;
    /// Write all the items of an iterator to a Ws2811 strip
    fn write<T, I>(&mut self, iterator: T) -> Result<(), E>
    where
        T: Iterator<Item = I>,
        I: Into<Self::Color>,
    {
        // We introduce an offset in the fifo here, so there's always one byte in transit
        // Some MCUs (like the stm32f1) only a one byte fifo, which would result
        // in overrun error if two bytes need to be stored
        block!(self.spi.send(0))?;
        if cfg!(feature = "mosi_idle_high") {
            self.flush()?;
        }

        for item in iterator {
            let item = item.into();
            self.write_byte(item.r)?;
            self.write_byte(item.b)?;
            self.write_byte(item.g)?;
        }
        self.flush()?;
        // Now, resolve the offset we introduced at the beginning
        block!(self.spi.read())?;
        Ok(())
    }
}