pub struct Layer;
impl<S> tracing_subscriber::Layer<S> for Layer where S: tracing::Subscriber {
    fn enabled(&self, metadata: &tracing::Metadata<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) -> bool {
        true
    }
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = Visitor { dispatch_error: None };
        event.record(&mut visitor);
    }
}
impl Layer {
    pub fn new() -> Self {
        Self
    }
}


struct Visitor { dispatch_error: Option<Box<crate::status_backend::error::DispatchError>> }
impl tracing::field::Visit for Visitor {
    fn record_error(&mut self, field: &tracing::field::Field, value: &(dyn std::error::Error + 'static)) {
        dbg!("meow");
        if field.name() != "error" { return; }
        if let Some(error) = value.downcast_ref::<crate::status_backend::error::DispatchError>() {
            // self.dispatch_error = Some(core::ptr::addr_of!(*error))
        }   
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "error_box_ptr" {
            let ptr = u64::from_str_radix(&format!("{value:?}")["0x".len()..], 16).expect("failed to parse pointer");
            let ptr = ptr as *mut crate::status_backend::error::DispatchError;
            self.dispatch_error = Some(unsafe { Box::from_raw(ptr) });
        }
    }

}

