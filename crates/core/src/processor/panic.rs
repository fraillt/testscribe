use std::any::Any;

pub fn extract_string_from_panic_payload(payload: &(dyn Any + Send)) -> Option<String> {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(ToString::to_string))
}
