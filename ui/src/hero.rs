use dioxus::prelude::*;

use crate::components::select::{
    Select, SelectGroup, SelectGroupLabel, SelectItemIndicator, SelectList, SelectOption,
    SelectTrigger, SelectValue,
};

#[component]
pub fn Hero() -> Element {
    rsx! {
        NumberPicker {  }
    }
}

#[component]
pub fn NumberPicker() -> Element {
    let mut stored_id = use_signal(|| Some(Some(0_u32)));

    rsx! {

        div {
            id: "hero",

            Select::<u32> {
                value: stored_id,
                on_value_change: move |v| {
                    stored_id.set(Some(v));
                },
                SelectTrigger {
                    // The (optional) select value displays the currently selected text value.
                    SelectValue {}
                }
                // All groups must be wrapped in the select list.
                SelectList {
                    // An group within the select dropdown which may contain multiple items.
                    SelectGroup {
                        // The label for the group
                        SelectGroupLabel {
                            "Other"
                        }
                        // // Each select option represents an individual option in the dropdown. The type must match the type of the select.
                        SelectOption::<u32> {
                            // The value of the item, which will be passed to the on_value_change callback when selected.
                            value: 10_u32,
                            text_value: "Dings",
                            index: 0_usize,
                            "Dings"
                            // Select item indicator is only rendered if the item is selected.
                            SelectItemIndicator {}
                        }
                        // // Each select option represents an individual option in the dropdown. The type must match the type of the select.
                        SelectOption::<u32> {
                            // The value of the item, which will be passed to the on_value_change callback when selected.
                            value: 20_u32,
                            text_value: "Andere Dings",
                            index: 1_usize,
                            "Andere Dings"
                            // Select item indicator is only rendered if the item is selected.
                            SelectItemIndicator {}
                        }
                    }
                }
            }
        }
    }
}
