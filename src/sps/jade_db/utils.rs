// utils.rs

use chrono::{DateTime, NaiveDateTime};
use sqlx::types::time::{OffsetDateTime, PrimitiveDateTime};
use time::Duration;

pub type Error = Box<dyn core::error::Error>;
pub type Result<T> = core::result::Result<T, Error>;

pub struct JadeDateNaive(NaiveDateTime);

impl From<NaiveDateTime> for JadeDateNaive {
    fn from(item: NaiveDateTime) -> Self {
        JadeDateNaive(item)
    }
}

impl From<JadeDateNaive> for NaiveDateTime {
    fn from(item: JadeDateNaive) -> Self {
        item.0
    }
}

pub struct JadeDatePrimitive(PrimitiveDateTime);

impl From<PrimitiveDateTime> for JadeDatePrimitive {
    fn from(item: PrimitiveDateTime) -> Self {
        JadeDatePrimitive(item)
    }
}

impl From<JadeDatePrimitive> for PrimitiveDateTime {
    fn from(item: JadeDatePrimitive) -> Self {
        item.0
    }
}

impl From<PrimitiveDateTime> for JadeDateNaive {
    fn from(value: PrimitiveDateTime) -> Self {
        let nanos = value.assume_utc().unix_timestamp_nanos();
        let nanos_i64 = nanos.try_into().unwrap();
        let datetime_utc = DateTime::from_timestamp_nanos(nanos_i64);
        datetime_utc.naive_utc().into()
    }
}

impl From<NaiveDateTime> for JadeDatePrimitive {
    fn from(value: NaiveDateTime) -> Self {
        let date_time = value.and_utc();
        let millis = date_time.timestamp_millis();
        let offset_datetime = OffsetDateTime::UNIX_EPOCH + Duration::milliseconds(millis);
        let date = offset_datetime.date();
        let time = offset_datetime.time();
        PrimitiveDateTime::new(date, time).into()
    }
}

pub fn convert_primitive_date_time_to_naive_date_time(
    primitive_date_time: &PrimitiveDateTime,
) -> NaiveDateTime {
    let nanos = primitive_date_time.assume_utc().unix_timestamp_nanos();
    let nanos_i64 = nanos.try_into().unwrap();
    let datetime_utc = DateTime::from_timestamp_nanos(nanos_i64);
    datetime_utc.naive_utc()
}

pub fn convert_naive_date_time_to_primitive_date_time(
    naive_date_time: &NaiveDateTime,
) -> PrimitiveDateTime {
    let date_time = naive_date_time.and_utc();
    let millis = date_time.timestamp_millis();
    let offset_datetime = OffsetDateTime::UNIX_EPOCH + Duration::milliseconds(millis);
    let date = offset_datetime.date();
    let time = offset_datetime.time();
    PrimitiveDateTime::new(date, time)
}

//---------------------------------------------------------------------------
//---------------------------------------------------------------------------
//---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::types::time::{Date, Time};
    use time::Month;

    #[test]
    fn test_always_succeed() {
        assert!(true);
    }

    #[test]
    fn test_convert_primitive_date_time_to_naive_date_time() -> Result<()> {
        let date = Date::from_calendar_date(2024, Month::December, 19)?;
        let time = Time::from_hms_milli(12, 34, 56, 789)?;
        let primitive = PrimitiveDateTime::new(date, time);
        let naive = convert_primitive_date_time_to_naive_date_time(&primitive);
        let dt = naive.and_utc();
        assert_eq!(1734611696789000000, dt.timestamp_nanos_opt().unwrap());
        Ok(())
    }

    #[test]
    fn test_convert_naive_date_time_to_primitive_date_time() -> Result<()> {
        let dt = DateTime::from_timestamp_millis(1734611696789).unwrap();
        let naive = dt.naive_utc();
        let primitive = convert_naive_date_time_to_primitive_date_time(&naive);
        assert_eq!(
            1734611696789000000,
            primitive.assume_utc().unix_timestamp_nanos()
        );
        Ok(())
    }
}
