use super::*;

/// Modbus client generator/processor
///
/// One object can be used for multiple calls
pub struct ModbusRequest {
    /// transaction id, (TCP/UDP only), default: 1. To change, set the value manually
    pub tr_id: u16,
    pub unit_id: u8,
    pub func: u8,
    pub reg: u16,
    pub count: u16,
    pub proto: ModbusProto,
}

impl ModbusRequest {
    /// Crate new Modbus client
    pub fn new(unit_id: u8, proto: ModbusProto) -> Self {
        Self {
            tr_id: 1,
            unit_id: unit_id,
            func: 0,
            reg: 0,
            count: 0,
            proto: proto,
        }
    }

    pub fn generate_get_coils<V: VectorTrait<u8>>(
        &mut self,
        reg: u16,
        count: u16,
        request: &mut V,
    ) -> Result<(), ErrorKind> {
        self.reg = reg;
        self.count = count;
        self.func = MODBUS_GET_COILS;
        return self.generate(&[], request);
    }

    pub fn generate_get_discretes<V: VectorTrait<u8>>(
        &mut self,
        reg: u16,
        count: u16,
        request: &mut V,
    ) -> Result<(), ErrorKind> {
        self.reg = reg;
        self.count = count;
        self.func = MODBUS_GET_DISCRETES;
        return self.generate(&[], request);
    }

    pub fn generate_get_holdings<V: VectorTrait<u8>>(
        &mut self,
        reg: u16,
        count: u16,
        request: &mut V,
    ) -> Result<(), ErrorKind> {
        self.reg = reg;
        self.count = count;
        self.func = MODBUS_GET_HOLDINGS;
        return self.generate(&[], request);
    }

    pub fn generate_get_inputs<V: VectorTrait<u8>>(
        &mut self,
        reg: u16,
        count: u16,
        request: &mut V,
    ) -> Result<(), ErrorKind> {
        self.reg = reg;
        self.count = count;
        self.func = MODBUS_GET_INPUTS;
        return self.generate(&[], request);
    }

    pub fn generate_set_coil<V: VectorTrait<u8>>(
        &mut self,
        reg: u16,
        value: bool,
        request: &mut V,
    ) -> Result<(), ErrorKind> {
        self.reg = reg;
        self.count = 1;
        self.func = MODBUS_SET_COIL;
        return self.generate(
            &[
                match value {
                    true => 0xff,
                    false => 0x00,
                },
                0x00,
            ],
            request,
        );
    }

    pub fn generate_set_holding<V: VectorTrait<u8>>(
        &mut self,
        reg: u16,
        value: u16,
        request: &mut V,
    ) -> Result<(), ErrorKind> {
        self.reg = reg;
        self.count = 1;
        self.func = MODBUS_SET_HOLDING;
        return self.generate(&value.to_be_bytes(), request);
    }

    pub fn generate_set_holdings_bulk<V: VectorTrait<u8>>(
        &mut self,
        reg: u16,
        values: &[u16],
        request: &mut V,
    ) -> Result<(), ErrorKind> {
        if values.len() > 125 {
            return Err(ErrorKind::OOB);
        }
        self.reg = reg;
        self.count = values.len() as u16;
        self.func = MODBUS_SET_HOLDINGS_BULK;
        let mut data: ModbusFrameBuf = [0; 256];
        let mut pos = 0;
        for v in values {
            data[pos] = (v >> 8) as u8;
            data[pos + 1] = *v as u8;
            pos = pos + 2;
        }
        return self.generate(&data[..values.len() * 2], request);
    }

    pub fn generate_set_holdings_string<V: VectorTrait<u8>>(
        &mut self,
        reg: u16,
        values: &str,
        request: &mut V,
    ) -> Result<(), ErrorKind> {
        let values = values.as_bytes();
        let length = values.len() + values.len() % 2;
        if length > 250 {
            return Err(ErrorKind::OOB);
        }
        self.reg = reg;
        self.count = length as u16 / 2u16;
        self.func = MODBUS_SET_HOLDINGS_BULK;
        let mut data: ModbusFrameBuf = [0; 256];
        let mut pos = 0;
        for v in values {
            data[pos] = *v;
            pos +=1;
        }
        return self.generate(&data[..length], request);
    }

    pub fn generate_set_coils_bulk<V: VectorTrait<u8>>(
        &mut self,
        reg: u16,
        values: &[bool],
        request: &mut V,
    ) -> Result<(), ErrorKind> {
        if values.len() > 4000 {
            return Err(ErrorKind::OOB);
        }
        self.reg = reg;
        self.count = values.len() as u16;
        self.func = MODBUS_SET_COILS_BULK;
        let mut data: ModbusFrameBuf = [0; 256];
        let mut pos = 0;
        let mut cbyte = 0;
        let mut bidx = 0;
        for v in values {
            if *v {
                cbyte = cbyte | 1 << bidx;
            }
            bidx = bidx + 1;
            if bidx > 7 {
                bidx = 0;
                data[pos] = cbyte;
                pos = pos + 1;
                cbyte = 0;
            }
        }
        let len;
        if bidx > 0 {
            data[pos] = cbyte;
            len = pos + 1;
        } else {
            len = pos;
        }
        return self.generate(&data[..len], request);
    }

    fn parse_response(&self, buf: &[u8]) -> Result<(usize, usize), ErrorKind> {
        let (frame_start, frame_end) = match self.proto {
            ModbusProto::TcpUdp => {
                if buf.len() < 9 {
                    return Err(ErrorKind::FrameBroken);
                }
                let tr_id = u16::from_be_bytes([buf[0], buf[1]]);
                let proto = u16::from_be_bytes([buf[2], buf[3]]);
                if tr_id != self.tr_id || proto != 0 {
                    return Err(ErrorKind::FrameBroken);
                }
                (6, buf.len())
            }
            ModbusProto::Rtu => {
                if buf.len() < 5 {
                    return Err(ErrorKind::FrameBroken);
                }
                let len = buf.len();
                let crc = calc_crc16(buf, len as u8 - 2);
                if crc != u16::from_le_bytes([buf[len - 2], buf[len - 1]]) {
                    return Err(ErrorKind::FrameCRCError);
                }
                (0, buf.len() - 2)
            }
            ModbusProto::Ascii => {
                if buf.len() < 4 {
                    return Err(ErrorKind::FrameBroken);
                }
                let len = buf.len();
                let lrc = calc_lrc(buf, len as u8 - 1);
                if lrc != buf[len - 1] {
                    return Err(ErrorKind::FrameCRCError);
                }
                (0, buf.len() - 1)
            }
        };
        let unit_id = buf[frame_start];
        let func = buf[frame_start + 1];
        if unit_id != self.unit_id {
            return Err(ErrorKind::FrameBroken);
        }
        if func != self.func {
            // func-0x80 but some servers respond any shit
            return Err(ErrorKind::from_modbus_error(buf[frame_start + 2]));
        }
        if self.func > 0 && self.func < 5 {
            let len = buf[frame_start + 2] as usize;
            if len * 2 < (frame_end - frame_start) - 3 {
                return Err(ErrorKind::FrameBroken);
            }
        }
        return Ok((frame_start, frame_end));
    }

    /// Parse response and make sure there's no Modbus error inside
    ///
    /// The input buffer SHOULD be cut to actual response length
    pub fn parse_ok(&self, buf: &[u8]) -> Result<(), ErrorKind> {
        match self.parse_response(buf) {
            Ok(_) => return Ok(()),
            Err(e) => return Err(e),
        };
    }

    /// Parse response, make sure there's no Modbus error inside, plus parse response data as u16
    /// (getting holdings, inputs)
    ///
    /// The input buffer SHOULD be cut to actual response length
    pub fn parse_u16<V: VectorTrait<u16>>(
        &self,
        buf: &[u8],
        result: &mut V,
    ) -> Result<(), ErrorKind> {
        let (frame_start, frame_end) = match self.parse_response(buf) {
            Ok(v) => v,
            Err(e) => return Err(e),
        };
        let mut pos = frame_start + 3;
        while pos < frame_end - 1 {
            let value = u16::from_be_bytes([buf[pos], buf[pos + 1]]);
            if result.get_len() >= self.count as usize {
                break;
            }
            if result.add(value).is_err() {
                return Err(ErrorKind::OOB);
            }
            pos = pos + 2;
        }
        Ok(())
    }

    /// Parse response, make sure there's no Modbus error inside, plus parse response data as u16
    /// (getting holdings, inputs)
    ///
    /// The input buffer SHOULD be cut to actual response length
    pub fn parse_string(
        &self,
        buf: &[u8],
        result: &mut String,
    ) -> Result<(), ErrorKind> {
        let (frame_start, frame_end) = match self.parse_response(buf) {
            Ok(v) => v,
            Err(e) => return Err(e),
        };
        let val = &buf[frame_start + 3 .. frame_end];
        let vl = val.iter()
            .position(|&c| c == b'\0')
            .unwrap_or(val.len());
        *result = match std::str::from_utf8(&val[..vl]) {
            Ok(v) => v.to_string(),
            Err(e) => return Err(ErrorKind::Utf8Error),
        };
        Ok(())
    }

    /// Parse response, make sure there's no Modbus error inside, plus parse response data as bools
    /// (getting coils, discretes)
    ///
    /// The input buffer SHOULD be cut to actual response length
    pub fn parse_bool<V: VectorTrait<bool>>(
        &self,
        buf: &[u8],
        result: &mut V,
    ) -> Result<(), ErrorKind> {
        let (frame_start, frame_end) = match self.parse_response(buf) {
            Ok(v) => v,
            Err(e) => return Err(e),
        };
        for pos in frame_start + 3..frame_end {
            let b = buf[pos];
            for i in 0..8 {
                if result.get_len() >= self.count as usize {
                    break;
                }
                if result.add(b >> i & 1 == 1).is_err() {
                    return Err(ErrorKind::OOB);
                }
            }
        }
        Ok(())
    }

    fn generate<V: VectorTrait<u8>>(&self, data: &[u8], request: &mut V) -> Result<(), ErrorKind> {
        request.clear_all();
        if self.proto == ModbusProto::TcpUdp {
            if request.add_bulk(&self.tr_id.to_be_bytes()).is_err() {
                return Err(ErrorKind::OOB);
            }
            if request.add_bulk(&[0u8, 0, 0, 0]).is_err() {
                return Err(ErrorKind::OOB);
            }
        }
        if request.add_bulk(&[self.unit_id, self.func]).is_err() {
            return Err(ErrorKind::OOB);
        }
        if request.add_bulk(&self.reg.to_be_bytes()).is_err() {
            return Err(ErrorKind::OOB);
        }
        match self.func {
            MODBUS_GET_COILS | MODBUS_GET_DISCRETES | MODBUS_GET_HOLDINGS | MODBUS_GET_INPUTS => {
                if request.add_bulk(&self.count.to_be_bytes()).is_err() {
                    return Err(ErrorKind::OOB);
                }
            }
            MODBUS_SET_COIL | MODBUS_SET_HOLDING => {
                for v in data {
                    if request.add(*v).is_err() {
                        return Err(ErrorKind::OOB);
                    }
                }
            }
            MODBUS_SET_COILS_BULK | MODBUS_SET_HOLDINGS_BULK => {
                if request.add_bulk(&self.count.to_be_bytes()).is_err() {
                    return Err(ErrorKind::OOB);
                }
                if request.add(data.len() as u8).is_err() {
                    return Err(ErrorKind::OOB);
                }
                for v in data {
                    if request.add(*v).is_err() {
                        return Err(ErrorKind::OOB);
                    }
                }
            }
            _ => unimplemented!(),
        };
        match self.proto {
            ModbusProto::TcpUdp => {
                let len = ((request.get_len() as u16) - 6).to_be_bytes();
                request.replace(4, len[0]);
                request.replace(5, len[1]);
            }
            ModbusProto::Rtu => {
                let crc = calc_crc16(request.get_slice(), request.get_len() as u8);
                if request.add_bulk(&crc.to_le_bytes()).is_err() {
                    return Err(ErrorKind::OOB);
                }
            }
            ModbusProto::Ascii => {
                let lrc = calc_lrc(request.get_slice(), request.get_len() as u8);
                if request.add(lrc).is_err() {
                    return Err(ErrorKind::OOB);
                }
            }
        };
        Ok(())
    }
}
