use chrono::{DateTime, Datelike, Local, Month, Months, Timelike, Utc, Weekday};
use db::Ulid;
use dioxus::prelude::*;
use dioxus_free_icons::{
    icons::fa_solid_icons::{FaAngleLeft, FaAngleRight},
    Icon,
};

#[component]
pub fn DateTimePicker(date_time: DateTime<Utc>, on_input: Callback<DateTime<Utc>, ()>) -> Element {
    let popover_id = use_signal(|| Ulid::new().to_string());

    let local_date = DateTime::<Local>::from(date_time);

    rsx! {
        document::Link { rel: "stylesheet", href: asset!("./style.css") }

        div {
            class: "dtp-button",
            style: "anchor-name: --{popover_id()}",

            button {
                class: "dtp-button",
                popovertarget: "date-{popover_id()}",
                {local_date.format("%d.%m.%Y").to_string()}
            }

            button {
                class: "dtp-button",
                popovertarget: "time-{popover_id()}",
                {local_date.format("%H:%M").to_string()}
            }
        }

        div {
            popover: "auto",
            id: "date-{popover_id()}",
            class: "dtp-popover",
            style: "position-anchor: --{popover_id()}",

            CalendarTable { date_time, on_input }
        }

        div {
            popover: "auto",
            id: "time-{popover_id()}",
            class: "dtp-popover",
            style: "position-anchor: --{popover_id()}",

            TimeInput { date_time, on_input }
        }
    }
}

#[component]
fn TimeInput(date_time: DateTime<Utc>, on_input: Callback<DateTime<Utc>, ()>) -> Element {
    let get_local_hour = move || DateTime::<Local>::from(date_time).hour().to_string();
    let get_local_min = move || DateTime::<Local>::from(date_time).minute().to_string();

    let mut hour_value = use_signal(get_local_hour);

    let mut set_hour_value = move |s: String| {
        let hour = s.parse();
        hour_value.set(s);
        if let Ok(hour) = hour {
            if let Some(new_date) = map_dt_in_local_tz(date_time, |dt| dt.with_hour(hour)) {
                on_input(new_date);
            }
        }
    };

    let mut reset_hour_value_str = move || {
        hour_value.set(get_local_hour());
    };

    let mut min_value = use_signal(get_local_min);

    let mut set_min_value = move |s: String| {
        let min = s.parse();
        min_value.set(s);
        if let Ok(min) = min {
            if let Some(new_date) = map_dt_in_local_tz(date_time, |dt| dt.with_minute(min)) {
                on_input(new_date);
            }
        }
    };

    let mut reset_min_value_str = move || {
        min_value.set(get_local_min());
    };

    rsx! {
        div {
            class: "dtp-time",

            input {
                value: "{hour_value}",
                oninput: move |ev| set_hour_value(ev.value()),
                onblur: move |_| reset_hour_value_str(),
            }

            ":"

            input {
                value: "{min_value}",
                oninput: move |ev| set_min_value(ev.value()),
                onblur: move |_| reset_min_value_str(),
            }
        }
    }
}

#[component]
fn CalendarTable(date_time: DateTime<Utc>, on_input: Callback<DateTime<Utc>, ()>) -> Element {
    let today_date = Local::now().date_naive();

    let local_date_time = DateTime::<Local>::from(date_time);
    let date = local_date_time.date_naive();

    let first_day = date.with_day(1).unwrap();

    let weeks_iter = first_day
        .iter_weeks()
        .map(|date| date.week(Weekday::Mon))
        .take_while(|week| {
            week.first_day().month() == date.month() || week.last_day().month() == date.month()
        })
        .map(|week| {
            let last_day = week.last_day();
            week.first_day()
                .iter_days()
                .take_while(move |d| d <= &last_day)
        });

    let current_month = Month::try_from(u8::try_from(date.month()).unwrap()).ok();
    let month_name = current_month.map(|m| m.name()).unwrap_or("???");

    let weekdays = [
        Weekday::Mon,
        Weekday::Tue,
        Weekday::Wed,
        Weekday::Thu,
        Weekday::Fri,
        Weekday::Sat,
        Weekday::Sun,
    ];

    let add_months = |dt: DateTime<Utc>, months: i32| {
        if months > 0 {
            map_dt_in_local_tz(dt, |dt| dt.checked_add_months(Months::new(months as u32))).unwrap()
        } else {
            map_dt_in_local_tz(dt, |dt| dt.checked_sub_months(Months::new(-months as u32))).unwrap()
        }
    };

    rsx! {
        div {
            class: "dtp-calendar",

            div {
                class: "dtp-header",

                div {
                    class: "dtp-pill",

                    button {
                        onclick: move |_| {on_input(add_months(date_time, -1))},
                        Icon {
                            width: 12,
                            height: 12,
                            icon: FaAngleLeft,
                        }
                    }

                    span {
                        {month_name}
                    }

                    button {
                        onclick: move |_| {on_input(add_months(date_time, 1))},
                        Icon {
                            width: 12,
                            height: 12,
                            icon: FaAngleRight,
                        }
                    }
                }


                div {
                    class: "dtp-pill",

                    button {
                        onclick: move |ev| {
                            let mul = if ev.modifiers().shift() {10} else {1};
                            on_input(add_months(date_time, -12 * mul));
                        },
                        Icon {
                            width: 12,
                            height: 12,
                            icon: FaAngleLeft,
                        }
                    }

                    span {
                        "{date.year()}"
                    }


                    button {
                        onclick: move |ev| {
                            let mul = if ev.modifiers().shift() {10} else {1};
                            on_input(add_months(date_time, 12 * mul));
                        },
                        Icon {
                            width: 12,
                            height: 12,
                            icon: FaAngleRight,
                        }
                    }
                }

            }

            div {
                class: "dtp-table",

                for weekday in weekdays {
                    span {
                        "{weekday}"
                    }
                }

                for week in weeks_iter {
                    for day in week {
                        button {
                            class: "date-picker-select-button",
                            class: if day.month() != date.month() {"other-month"},
                            class: if day == date {"selected"},
                            class: if day == today_date {"today"},
                            key: "{day}",
                            onclick: move |_| on_input(
                                map_dt_in_local_tz(date_time, |dt| {
                                    day.and_time(dt.time()).and_local_timezone(Local).single()
                                }).unwrap()
                            ),
                            "{day.day()}"
                        }
                    }
                }

            }
        }
    }
}

fn map_dt_in_local_tz(
    dt: DateTime<Utc>,
    f: impl FnOnce(DateTime<Local>) -> Option<DateTime<Local>>,
) -> Option<DateTime<Utc>> {
    f(dt.into()).map(Into::into)
}
