use std::fmt::Debug;

use rmp::{decode::RmpRead, encode::RmpWrite};

#[derive(Clone, Debug)]
pub enum Data {
    Bool(bool),
    F64(f64),
    I32(i32),
    F32(f32),
    String(String),
    BoolArray(Vec<bool>),
    F64Array(Vec<f64>),
    I32Array(Vec<i32>),
    F32Array(Vec<f32>),
    StringArray(Vec<String>),
}
impl Data {
    pub fn from<R: RmpRead>(rd: &mut R, data_type: u8) -> Result<Self, ()> {
        Ok(match data_type {
            <bool as DataWrap>::MSGPCK => Self::Bool(<bool as DataWrap>::decode(rd)?),
            <f64 as DataWrap>::MSGPCK => Self::F64(<f64 as DataWrap>::decode(rd)?),
            <i32 as DataWrap>::MSGPCK => Self::I32(<i32 as DataWrap>::decode(rd)?),
            <f32 as DataWrap>::MSGPCK => Self::F32(<f32 as DataWrap>::decode(rd)?),
            <String as DataWrap>::MSGPCK => Self::String(<String as DataWrap>::decode(rd)?),
            <Vec<bool> as DataWrap>::MSGPCK => {
                Self::BoolArray(<Vec<bool> as DataWrap>::decode(rd)?)
            }
            <Vec<f64> as DataWrap>::MSGPCK => Self::F64Array(<Vec<f64> as DataWrap>::decode(rd)?),
            <Vec<i32> as DataWrap>::MSGPCK => Self::I32Array(<Vec<i32> as DataWrap>::decode(rd)?),
            <Vec<f32> as DataWrap>::MSGPCK => Self::F32Array(<Vec<f32> as DataWrap>::decode(rd)?),
            <Vec<String> as DataWrap>::MSGPCK => {
                Self::StringArray(<Vec<String> as DataWrap>::decode(rd)?)
            }
            _ => return Err(()),
        })
    }
}

pub trait DataWrap: Sized + Debug {
    const MSGPCK: u8;
    const STRING: &'static str;

    fn decode<R: RmpRead>(rd: &mut R) -> Result<Self, ()>;
    fn encode<W: RmpWrite>(wr: &mut W, val: Self) -> Result<(), ()>;
}
impl<T: DataType + Debug> DataWrap for T {
    const MSGPCK: u8 = Self::DATATYPE_MSGPCK;
    const STRING: &'static str = Self::DATATYPE_STRING;

    fn decode<R: RmpRead>(rd: &mut R) -> Result<Self, ()> {
        Self::decode(rd)
    }
    fn encode<W: RmpWrite>(wr: &mut W, val: Self) -> Result<(), ()> {
        Self::encode(wr, val)
    }
}
impl<T: DataType + Debug> DataWrap for Vec<T> {
    const MSGPCK: u8 = T::ARRAYDATATYPE_MSGPCK;
    const STRING: &'static str = T::ARRAYDATATYPE_STRING;

    fn decode<R: RmpRead>(rd: &mut R) -> Result<Self, ()> {
        T::decode_array(rd)
    }
    fn encode<W: RmpWrite>(wr: &mut W, val: Self) -> Result<(), ()> {
        T::encode_array(wr, val)
    }
}

pub trait DataType: Sized {
    const DATATYPE_MSGPCK: u8;
    const ARRAYDATATYPE_MSGPCK: u8;
    const DATATYPE_STRING: &'static str;
    const ARRAYDATATYPE_STRING: &'static str;

    fn decode<R: RmpRead>(rd: &mut R) -> Result<Self, ()>;
    fn encode<W: RmpWrite>(wr: &mut W, val: Self) -> Result<(), ()>;

    fn decode_array<R: RmpRead>(rd: &mut R) -> Result<Vec<Self>, ()> {
        let mut buf = Vec::new();

        let len = rmp::decode::read_array_len(rd).map_err(|_| ())?;

        for _ in 0..len {
            buf.push(Self::decode(rd)?);
        }

        Ok(buf)
    }

    fn encode_array<W: RmpWrite>(wr: &mut W, vals: Vec<Self>) -> Result<(), ()> {
        rmp::encode::write_array_len(wr, vals.len() as u32).map_err(|_| ())?;

        for val in vals {
            Self::encode(wr, val)?;
        }

        Ok(())
    }
}

impl DataType for bool {
    const DATATYPE_MSGPCK: u8 = 0;
    const ARRAYDATATYPE_MSGPCK: u8 = 16;
    const DATATYPE_STRING: &'static str = "boolean";
    const ARRAYDATATYPE_STRING: &'static str = "boolean[]";

    fn decode<R: RmpRead>(rd: &mut R) -> Result<Self, ()> {
        let ret = rmp::decode::read_bool(rd).map_err(|_| ())?;

        Ok(ret)
    }

    fn encode<W: RmpWrite>(wr: &mut W, val: Self) -> Result<(), ()> {
        rmp::encode::write_bool(wr, val).map_err(|_| ())?;

        Ok(())
    }
}
impl DataType for f64 {
    const DATATYPE_MSGPCK: u8 = 1;
    const ARRAYDATATYPE_MSGPCK: u8 = 17;
    const DATATYPE_STRING: &'static str = "double";
    const ARRAYDATATYPE_STRING: &'static str = "double[]";

    fn decode<R: RmpRead>(rd: &mut R) -> Result<Self, ()> {
        let ret = rmp::decode::read_f64(rd).map_err(|_| ())?;

        Ok(ret)
    }

    fn encode<W: RmpWrite>(wr: &mut W, val: Self) -> Result<(), ()> {
        rmp::encode::write_f64(wr, val).map_err(|_| ())?;

        Ok(())
    }
}
impl DataType for i32 {
    const DATATYPE_MSGPCK: u8 = 2;
    const ARRAYDATATYPE_MSGPCK: u8 = 18;
    const DATATYPE_STRING: &'static str = "int";
    const ARRAYDATATYPE_STRING: &'static str = "int[]";

    fn decode<R: RmpRead>(rd: &mut R) -> Result<Self, ()> {
        let ret = rmp::decode::read_i32(rd).map_err(|_| ())?;

        Ok(ret)
    }

    fn encode<W: RmpWrite>(wr: &mut W, val: Self) -> Result<(), ()> {
        rmp::encode::write_i32(wr, val).map_err(|_| ())?;

        Ok(())
    }
}

impl DataType for f32 {
    const DATATYPE_MSGPCK: u8 = 3;
    const ARRAYDATATYPE_MSGPCK: u8 = 19;
    const DATATYPE_STRING: &'static str = "float";
    const ARRAYDATATYPE_STRING: &'static str = "float[]";

    fn decode<R: RmpRead>(rd: &mut R) -> Result<Self, ()> {
        let ret = rmp::decode::read_f32(rd).map_err(|_| ())?;

        Ok(ret)
    }

    fn encode<W: RmpWrite>(wr: &mut W, val: Self) -> Result<(), ()> {
        rmp::encode::write_f32(wr, val).map_err(|_| ())?;

        Ok::<(), _>(())
    }
}

impl DataType for String {
    const DATATYPE_MSGPCK: u8 = 4;
    const ARRAYDATATYPE_MSGPCK: u8 = 20;
    const DATATYPE_STRING: &'static str = "string";
    const ARRAYDATATYPE_STRING: &'static str = "string[]";

    fn decode<R: RmpRead>(rd: &mut R) -> Result<Self, ()> {
        let mut buf = Vec::new();

        let ret = rmp::decode::read_str(rd, &mut buf).map_err(|_| ())?;

        Ok(ret.to_string())
    }

    fn encode<W: RmpWrite>(wr: &mut W, val: Self) -> Result<(), ()> {
        rmp::encode::write_str(wr, &val).map_err(|_| ())?;

        Ok(())
    }
}
