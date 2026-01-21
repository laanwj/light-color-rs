use std::path::Path;

use linux_embedded_hal::{
    CdevPin,
    Delay,
    SpidevDevice,
    gpio_cdev::{Chip, LineRequestFlags},
    spidev::{SpiModeFlags, Spidev, SpidevOptions},
};
use rf24::{
    CrcLength,
    DataRate,
    PaLevel,
    radio::{
        RF24,
        prelude::{
            EsbAutoAck,
            EsbChannel,
            EsbCrcLength,
            EsbDataRate,
            EsbInit,
            EsbPaLevel,
            EsbPayloadLength,
            EsbPipe,
            EsbRadio,
            RadioErrorType,
        },
    },
};

/*
fn main() {
    let nrf24_ce_gpio = 25;
    let mut rf24 = rf24_init("/dev/spidev0.0", nrf24_ce_gpio).unwrap();

    let addr = 1; // 1 to 512
    let intensity = 10; // 0..100
    //let cct = 6; // 0..100, maps to 2700K .. 6000K
    //set_intensity_and_cct(&mut rf24, addr, intensity, cct).unwrap()
    let hue = 320;
    let sat = 100;
    //set_hue_sat_intensity(&mut rf24, addr, hue, sat, intensity).unwrap()
    set_intensity_cct_gm(&mut rf24, addr, 10, 20, 20).unwrap()
}
*/

pub fn rf24_init(
    spi_dev: impl AsRef<Path>,
    ce_gpio_offset: u32,
) -> Result<
    RF24<SpidevDevice, CdevPin, Delay>,
    <RF24<SpidevDevice, CdevPin, Delay> as RadioErrorType>::Error,
> {
    let mut spi = Spidev::open(spi_dev).unwrap();
    spi.configure(&SpidevOptions {
        bits_per_word: None,
        max_speed_hz: Some(8_000_000),
        lsb_first: None,
        spi_mode: Some(SpiModeFlags::SPI_MODE_0),
    })
    .unwrap();
    let spi_device = SpidevDevice(spi);

    let mut chip = Chip::new("/dev/gpiochip0").unwrap();
    let ce_line = chip.get_line(ce_gpio_offset).unwrap();
    let ce_pin = CdevPin::new(
        ce_line
            .request(LineRequestFlags::OUTPUT, 0, "rust-nanlite-nrf24")
            .unwrap(),
    )
    .unwrap();

    let mut rf24 = RF24::new(ce_pin, spi_device, Delay);

    rf24.init().unwrap();

    rf24.set_crc_length(CrcLength::Bit16)?;
    rf24.set_channel(0x73)?;
    rf24.set_pa_level(PaLevel::Max)?;

    rf24.set_dynamic_payloads(false)?;
    rf24.set_payload_length(4)?;
    rf24.set_auto_ack(true)?;
    rf24.set_data_rate(DataRate::Mbps1)?;
    rf24.set_address_length(5)?;

    Ok(rf24)
}

/*
pub fn set_intensity_and_cct<Radio: EsbRadio>(
    rf24: &mut Radio,
    addr: u16,
    intensity: u8,
    cct: u8,
) -> Result<(), Radio::Error> {
    let addr_bytes = addr.to_be_bytes();
    rf24.as_tx(Some(&[0x00, 0x00, 0x00, addr_bytes[0], addr_bytes[1]]))?;

    let intensity = intensity.min(100);
    let cct = cct.min(100);

    let check1 = intensity.overflowing_add(cct).0;
    let check2 = check1 ^ 0xff;

    rf24.send(&[intensity, cct, check1, check2], false)?;
    rf24.send(&[intensity, cct, check2, check1], false)?;
    rf24.send(&[intensity, cct, check1, check2], false)?;
    rf24.send(&[intensity, cct, check2, check1], false)?;

    Ok(())
}
*/

pub fn set_hue_sat_intensity<Radio: EsbRadio>(
    rf24: &mut Radio,
    addr: u16,
    hue: u16,
    sat: u8,
    intensity: u8,
) -> Result<(), Radio::Error> {
    let addr_bytes = addr.to_be_bytes();
    rf24.as_tx(Some(&[0x00, 0x00, 0x00, addr_bytes[0], addr_bytes[1]]))?;

    let hue = hue.min(360);
    let sat = sat.min(100);
    let intensity = intensity.min(100);

    rf24.send(&[0xf0 | ((hue >> 8) as u8), intensity, (hue & 0xff) as u8, sat], false)?;

    Ok(())
}

pub fn set_intensity_cct_gm<Radio: EsbRadio>(
    rf24: &mut Radio,
    addr: u16,
    intensity: u8,
    cct: u8,
    gm: u8,
) -> Result<(), Radio::Error> {
    let addr_bytes = addr.to_be_bytes();
    rf24.as_tx(Some(&[0x00, 0x00, 0x00, addr_bytes[0], addr_bytes[1]]))?;

    let intensity = intensity.min(100);
    let cct = cct.min(100);
    let gm = gm.min(100);
    let check = intensity.overflowing_add(cct).0;

    // Handle check != gm ^ 0xff requirement by fudging one of the values and recomputing.
    let (cct, check) = if check == gm ^ 0xff {
        let cct_new = if cct == 100 { 99 } else { cct + 1 };
        let check_new = intensity.overflowing_add(cct_new).0;
        (cct_new, check_new)
    } else {
        (cct, check)
    };

    rf24.send(&[intensity, cct, gm, check], false)?;

    Ok(())
}
