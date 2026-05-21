use pixmill_core::IbpError;
use wasm_bindgen::JsError;

// Flatten IbpError to its Display string for the JS side. Structured variants
// (Io / Decode / UnsupportedFormat / ...) aren't preserved across the boundary
// in this phase — the JS layer treats failures as opaque Error messages.
pub fn to_js_error(err: IbpError) -> JsError {
    JsError::new(&err.to_string())
}
