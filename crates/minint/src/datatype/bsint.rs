use rmp::{decode::RmpRead, encode::RmpWrite, Marker};

use crate::NtError;

use super::DataType;

#[derive(Clone, Debug)]
pub enum BsInt {
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
}
impl DataType for BsInt {
    const DATATYPE_MSGPCK: u8 = 2;
    const ARRAYDATATYPE_MSGPCK: u8 = 18;
    const DATATYPE_STRING: &'static str = "int";
    const ARRAYDATATYPE_STRING: &'static str = "int[]";

    fn decode<R: RmpRead>(rd: &mut R) -> Result<Self, ()> {
        let marker = rmp::decode::read_marker(rd).map_err(|_| ())?;
        match marker {
            Marker::I8 => {
                Ok(BsInt::I8(rmp::decode::read_i8(rd).map_err(|_| ())?))
            }
            Marker::I16 => {
                Ok(BsInt::I16(rmp::decode::read_i16(rd).map_err(|_| ())?))
            }
            Marker::I32 => {
                Ok(BsInt::I32(rmp::decode::read_i32(rd).map_err(|_| ())?))
            }
            Marker::I64 => {
                Ok(BsInt::I64(rmp::decode::read_i64(rd).map_err(|_| ())?))
            }
            Marker::U8 => {
                Ok(BsInt::U8(rmp::decode::read_u8(rd).map_err(|_| ())?))
            }
            Marker::U16 => {
                Ok(BsInt::U16(rmp::decode::read_u16(rd).map_err(|_| ())?))
            }
            Marker::U32 => {
                Ok(BsInt::U32(rmp::decode::read_u32(rd).map_err(|_| ())?))
            }
            Marker::U64 => {
                Ok(BsInt::U64(rmp::decode::read_u64(rd).map_err(|_| ())?))
            }
            //_ => Err(NtError::InvalidIntMarker),
            _ => Err(()),
        }
    }

    fn encode<W: RmpWrite>(wr: &mut W, val: Self) -> Result<(), ()> {
        use rmp::encode::*;
        let ret = match val {
            BsInt::I8(val) => {
                write_i8(wr, val)
            }
            BsInt::I16(val) => {
                write_i16(wr, val)
            }
            BsInt::I32(val) => {
                write_i32(wr, val)
            }
            BsInt::I64(val) => {
                write_i64(wr, val)
            }
            BsInt::U8(val) => {
                write_u8(wr, val)
            }
            BsInt::U16(val) => {
                write_u16(wr, val)
            }
            BsInt::U32(val) => {
                write_u32(wr, val)
            }
            BsInt::U64(val) => {
                write_u64(wr, val)
            }
        };

        ret.map_err(|_| ())?;

        Ok(())
    }
}

impl From<i8> for BsInt {
    fn from(val: i8) -> Self {
        BsInt::I8(val)
    }
}
impl From<i16> for BsInt {
    fn from(val: i16) -> Self {
        BsInt::I16(val)
    }
}
impl From<i32> for BsInt {
    fn from(val: i32) -> Self {
        BsInt::I32(val)
    }
}
impl From<i64> for BsInt {
    fn from(val: i64) -> Self {
        BsInt::I64(val)
    }
}
impl From<u8> for BsInt {
    fn from(val: u8) -> Self {
        BsInt::U8(val)
    }
}
impl From<u16> for BsInt {
    fn from(val: u16) -> Self {
        BsInt::U16(val)
    }
}
impl From<u32> for BsInt {
    fn from(val: u32) -> Self {
        BsInt::U32(val)
    }
}
impl From<u64> for BsInt {
    fn from(val: u64) -> Self {
        BsInt::U64(val)
    }
}


