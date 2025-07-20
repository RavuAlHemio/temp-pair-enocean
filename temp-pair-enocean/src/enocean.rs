//! EnOcean Serial Protocol 3 packet decoding routines.


use from_to_repr::from_to_other;
use stm32f7::stm32f745::Peripherals;

use crate::crc8::crc8;
use crate::i2c::{I2c, I2c2, I2cAddress};
use crate::uart::{Uart, Usart2};


const SYNC_BYTE: u8 = 0x55;

type EnoceanUart = Usart2;


#[derive(Clone, Copy, Debug)]
#[from_to_other(base_type = u8, derive_compare = "as_int")]
pub enum PacketType {
    RadioErp1 = 0x01,
    Response = 0x02,
    RadioSubTelegram = 0x03,
    Event = 0x04,
    CommonCommand = 0x05,
    SmartAcknowledgeCommand = 0x06,
    RemoteManagementCommand = 0x07,
    RadioMessage = 0x09,
    RadioErp2 = 0x0A,
    ConfigCommand = 0x0B,
    CommandAccepted = 0x0C,
    Raw802_15_4 = 0x10,
    Raw2_4 = 0x11,
    Other(u8),
}

#[derive(Clone, Copy, Debug)]
#[from_to_other(base_type = u8, derive_compare = "as_int")]
enum EventType {
    SmartAcknowledgeReclaimNotSucecssful = 0x01,
    SmartAcknowledgeConfirmLearn = 0x02,
    SmartAcknowledgeLearnAcknowledge = 0x03,
    Ready = 0x04,
    SecureDeviceEvent = 0x05,
    DutyCycleLimit = 0x06,
    TransmitFailed = 0x07,
    TxDone = 0x08,
    LearnModeDisabled = 0x09,
    Other(u8),
}

#[derive(Clone, Copy, Debug)]
#[from_to_other(base_type = u8, derive_compare = "as_int")]
enum CommonCommandType {
    WriteSleep = 0x01,
    WriteReset = 0x02,
    ReadVersion = 0x03,
    ReadSysLog = 0x04,
    WriteSysLog = 0x05,
    WriteBurnInSelfTest = 0x06,
    WriteIdBase = 0x07,
    ReadIdBase = 0x08,
    WriteRepeater = 0x09,
    ReadRepeater = 0x0A,
    WriteFilterAdd = 0x0B,
    WriteFilterDelete = 0x0C,
    WriteFilterClear = 0x0D,
    WriteFilterEnable = 0x0E,
    ReadFilter = 0x0F,
    WriteWaitMaturity = 0x10,
    WriteSubTelegram = 0x11,
    WriteMemory = 0x12,
    ReadMemory = 0x13,
    ReadMemoryAddress = 0x14,
    ReadSecurity = 0x15,
    WriteSecurity = 0x16,
    WriteLearnMode = 0x17,
    ReadLearnMode = 0x18,
    WriteSecureDeviceAdd = 0x19,
    WriteSecureDeviceDelete = 0x1A,
    ReadSecureDeviceByIndex = 0x1B,
    WriteMode = 0x1C,
    ReadNumberSecuredDevices = 0x1D,
    ReadSecureDeviceById = 0x1E,
    WriteSecureDeviceAddPsk = 0x1F,
    WriteSecureDeviceSendTeachIn = 0x20,
    WriteTemporaryRlcWindow = 0x21,
    ReadSecureDevicePsk = 0x22,
    ReadDutyCycleLimit = 0x23,
    SetBaudRate = 0x24,
    GetFrequencyInfo = 0x25,
    GetStepCode = 0x27,
    WriteRemoteManagementCode = 0x2E,
    WriteStartupDelay = 0x2F,
    WriteRemoteManagementRepeating = 0x30,
    ReadRemoteManagementRepeating = 0x31,
    SetNoiseThreshold = 0x32,
    GetNoiseThreshold = 0x33,
    WriteRlcSavePeriod = 0x36,
    WriteRlcLegacyMode = 0x37,
    WriteSecureDeviceV2Add = 0x38,
    ReadSecureDeviceV2ByIndex = 0x39,
    WriteRssiTestMode = 0x3A,
    ReadRssiTestMode = 0x3B,
    WriteSecureDeviceMaintenanceKey = 0x3C,
    ReadSecureDeviceMaintenanceKey = 0x3D,
    WriteTransparentMode = 0x3E,
    ReadTransparentMode = 0x3F,
    WriteTxOnlyMode = 0x40,
    ReadTxOnlyMode = 0x41,
    Other(u8),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Payload {
    buffer: [u8; 128],
    data_length: usize,
    optional_data_length: usize,
}
impl Payload {
    pub fn data(&self) -> &[u8] {
        &self.buffer[0..self.data_length]
    }

    pub fn optional_data(&self) -> &[u8] {
        &self.buffer[self.data_length..self.data_length+self.optional_data_length]
    }
}
impl Default for Payload {
    fn default() -> Self {
        Self {
            buffer: [0u8; 128],
            data_length: 0,
            optional_data_length: 0,
        }
    }
}


#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PacketResult {
    BufferEmpty,
    NotSynced,
    NotEnoughBytes,
    Short,
    Packet {
        packet_type: PacketType,
        payload: Payload,
    },
}


pub(crate) fn process_one_packet(peripherals: &Peripherals) -> PacketResult {
    // copy the current buffer contents
    let mut current_buffer = [0u8; 128];
    let original_size = EnoceanUart::copy_buffer(&mut current_buffer);
    if original_size == 0 {
        // empty buffer
        return PacketResult::BufferEmpty;
    }
    let original_slice = &current_buffer[..original_size];

    // find the sync byte
    let sync_byte_index_opt = original_slice.iter()
        .position(|b| *b == SYNC_BYTE);
    match sync_byte_index_opt {
        Some(sbi) => {
            // read the bytes before it, removing them from the ring buffer
            for _ in 0..sbi {
                let _ = EnoceanUart::take_byte();
            }
        },
        None => {
            // remove as many bytes as are in our slice
            for _ in 0..original_slice.len() {
                let _ = EnoceanUart::take_byte();
            }

            // there is no packet
            return PacketResult::NotSynced;
        },
    };

    // copy again now that we have gotten rid of a few bytes
    let current_size = EnoceanUart::copy_buffer(&mut current_buffer);
    let current_slice = &current_buffer[..current_size];

    // do we have enough bytes in the buffer for one whole packet?
    // minimum:
    // [0] sync
    // [1, 2] data length
    // [3] optional data length
    // [4] packet type
    // [5] crc8h
    // (data here)
    // (optional data here)
    // [6] crc8d
    // = 7 bytes
    if current_slice.len() < 7 {
        // not enough; try again later
        return PacketResult::NotEnoughBytes;
    }

    // check if the length values are plausible (CRC8)
    let calculated_crc8h = crc8(&current_slice[1..5]);
    if calculated_crc8h != current_slice[5] {
        // not actually the header

        // eat the sync byte and go around
        let _ = EnoceanUart::take_byte();
        return PacketResult::NotSynced;
    }

    // decode the length values
    let data_length =
        usize::from(current_slice[1]) << 8
        | usize::from(current_slice[2]);
    let optional_length = usize::from(current_slice[3]);

    // do we still have enough bytes?
    if current_slice.len() < 7 + data_length + optional_length {
        // no; try again later
        return PacketResult::Short;
    }

    // check data CRC
    let full_data_slice = &current_slice[6..6+data_length+optional_length];
    let calculated_crc8d = crc8(full_data_slice);
    if calculated_crc8d != current_slice[6+data_length+optional_length] {
        // nope

        // eat the sync byte and go around
        let _ = EnoceanUart::take_byte();
        return PacketResult::NotSynced;
    }

    // eat the whole packet
    for _ in 0..7+data_length+optional_length {
        let _ = EnoceanUart::take_byte();
    }

    let (data_slice, _optional_data_slice) = full_data_slice.split_at(data_length);

    // okay, what have we got?
    let packet_type = PacketType::from_base_type(current_slice[4]);
    match packet_type {
        PacketType::Event => {
            if data_slice.len() > 0 {
                // any interesting event?
                match EventType::from_base_type(data_slice[0]) {
                    EventType::Ready => {
                        // good morning! switch to transparent mode
                        let mut set_transparent_mode_packet = [
                            0x55, // sync byte
                            0x00, 0x02, // 2 bytes data length
                            0x00, // 0 bytes optional length
                            PacketType::CommonCommand.to_base_type(),
                            0x00, // CRC8H placeholder
                            CommonCommandType::WriteTransparentMode.to_base_type(),
                            0x01, // enable transparent mode
                            0x00, // CRC8D placeholder
                        ];
                        let crc8h = crc8(&set_transparent_mode_packet[1..5]);
                        let crc8d = crc8(&set_transparent_mode_packet[6..8]);
                        set_transparent_mode_packet[5] = crc8h;
                        set_transparent_mode_packet[8] = crc8d;

                        EnoceanUart::write(peripherals, &set_transparent_mode_packet);
                    },
                    _ => {},
                }
            }
        },
        _ => {},
    }

    // return the packet
    let mut payload_buffer = [0u8; 128];
    payload_buffer[..full_data_slice.len()].copy_from_slice(&full_data_slice);
    PacketResult::Packet {
        packet_type,
        payload: Payload {
            buffer: payload_buffer,
            data_length: data_length,
            optional_data_length: optional_length,
        },
    }
}
