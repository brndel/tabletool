use std::{ops::Sub, sync::Arc, time::Duration};

use bytepack::{BytePacker, FieldPacker, PackFormat};
use chrono::{DateTime, TimeDelta, Utc};
use db::{Db, Ulid};
use db_core::{
    defs::table::{TableData, TableDef, TableFieldDef},
    named::Named,
    record::RecordBytes,
    ty::FieldTy,
};
use dioxus::prelude::*;
use ui::{
    TableDialogButton,
    button::Button,
    table_tab_bar::{TableTab, TableTabBar},
};

use crate::Route;

#[component]
pub fn Home() -> Element {
    let db = use_context::<Db>();

    let mut reload_idx = use_signal(|| 0);

    let table_names = use_memo({
        let db = db.clone();
        move || {
            let _ = reload_idx();
            db.table_names()
        }
    });

    let on_submit = {
        let db = db.clone();
        move |table| {
            db.register_table(table);
            reload_idx.with_mut(|i| *i += 1);
        }
    };

    let mut init_db = {
        let db = db.clone();
        move || {
            let name_field = Arc::<str>::from("name");
            let project_group_name = Arc::<str>::from("project_group");
            let project_name = Arc::<str>::from("project");

            db.register_table(Named {
                name: project_group_name.clone(),
                value: TableDef {
                    fields: [Named::new(
                        name_field.clone(),
                        TableFieldDef {
                            ty: FieldTy::Text,
                            has_index: false,
                        },
                    )]
                    .into(),
                    main_display_field: Some(0),
                },
            });

            db.register_table(Named {
                name: project_name.clone(),
                value: TableDef {
                    fields: [
                        Named::new(
                            name_field.clone(),
                            TableFieldDef {
                                ty: FieldTy::Text,
                                has_index: false,
                            },
                        ),
                        Named::new(
                            "group",
                            TableFieldDef {
                                ty: FieldTy::RecordId {
                                    table_name: project_group_name,
                                },
                                has_index: false,
                            },
                        ),
                    ]
                    .into(),
                    main_display_field: Some(0),
                },
            });

            db.register_table(Named {
                name: "work_time".into(),
                value: TableDef {
                    fields: [
                        Named::new(
                            "project",
                            TableFieldDef {
                                ty: FieldTy::RecordId {
                                    table_name: project_name,
                                },
                                has_index: false,
                            },
                        ),
                        Named::new(
                            "start_time",
                            TableFieldDef {
                                ty: FieldTy::Timestamp,
                                has_index: false,
                            },
                        ),
                        Named::new(
                            "end_time",
                            TableFieldDef {
                                ty: FieldTy::Timestamp,
                                has_index: false,
                            },
                        ),
                        Named::new(
                            "notes",
                            TableFieldDef {
                                ty: FieldTy::Text,
                                has_index: false,
                            },
                        ),
                    ]
                    .into(),
                    main_display_field: None,
                },
            });

            fn create_record(
                db: &Db,
                table_name: &str,
                new_fn: impl FnOnce(&mut FieldPacker<'_, '_, TableData>),
            ) -> Ulid {
                let format = db.table(table_name).unwrap().as_ref().clone();
                let mut packer = BytePacker::new(format.fixed_byte_count());

                let mut fields = packer.fields(&format, 0);

                new_fn(&mut fields);

                let record = RecordBytes::create(packer.finish());

                db.insert_record(table_name, &record).unwrap();

                record.id()
            }

            let create_project_group = |name: &str| {
                create_record(&db, "project_group", |fields| {
                    fields.pack("name", name);
                })
            };

            let create_project = |name: &str, group: &Ulid| {
                create_record(&db, "project", |fields| {
                    fields.pack("name", name);
                    fields.pack("group", group);
                })
            };

            let create_worktime = |project: &Ulid,
                                   start_time: DateTime<Utc>,
                                   duration: Duration,
                                   notes: &str| {
                create_record(&db, "work_time", |fields| {
                    fields.pack("project", project);
                    fields.pack("start_time", &start_time);
                    fields.pack("end_time", &(start_time + duration));
                    fields.pack("notes", notes);
                })
            };

            let arbeit = create_project_group("Arbeit");
            let privat = create_project_group("Privat");
            let uni = create_project_group("Uni");

            let drink_manager = create_project("Getränkekasse", &arbeit);
            let video_tracking = create_project("Videos tracken", &arbeit);

            let tabletool = create_project("TableTool", &privat);
            let osm3d = create_project("Osm3D", &privat);

            let compilerdesign = create_project("Compilerdesign", &uni);
            let ba = create_project("Bachelorarbeit", &uni);

            let now = Utc::now();

            create_worktime(
                &drink_manager,
                now.sub(Duration::from_hours(2)),
                Duration::from_mins(60),
                "typst export geschrieben",
            );


            create_worktime(
                &drink_manager,
                now.sub(Duration::from_hours(24 + 5)),
                Duration::from_mins(120),
                "EntityId refactored",
            );

            reload_idx.with_mut(|x| *x += 1)
        }
    };

    rsx! {
        div {
            display: "flex",
            flex_direction: "row",
            gap: "0.5rem",
            align_items: "center",
            TableTabBar {
                for name in table_names.read().clone() {
                    TableTab {
                        to: Route::TablePage { name: name.to_string() },
                        "{name}"
                    }
                }
            }

            TableDialogButton { on_submit }
        }

        if table_names.with(|v| v.is_empty()) {
            Button {
                onclick: move |_| init_db(),
                "Init"
            }
        }
    }
}
