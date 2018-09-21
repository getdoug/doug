use chrono::{DateTime, Duration, Local, Utc};

pub fn datetime(time: DateTime<Utc>) -> String {
    time.with_timezone(&Local).format("%F %H:%M").to_string()
}

pub fn time(time: DateTime<Utc>) -> String {
    time.with_timezone(&Local).format("%H:%M").to_string()
}

pub fn duration(duration: Duration) -> String {
    let hours = duration.num_hours();
    let minutes = duration.num_minutes() % 60;
    let seconds = duration.num_seconds() % 60;

    if duration.num_minutes() == 0 {
        format!("{}s", seconds)
    } else if duration.num_hours() == 0 {
        format!(
            "{minutes}m {seconds:>2}s",
            minutes = minutes,
            seconds = seconds
        )
    } else {
        format!(
            "{hours}h {minutes:>2}m {seconds:>2}s",
            hours = hours,
            minutes = minutes,
            seconds = seconds
        )
    }
}
