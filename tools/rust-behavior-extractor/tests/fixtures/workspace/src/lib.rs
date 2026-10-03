pub mod worker;

pub trait Step { fn step(&self, value: u32) -> u32; }
pub trait Named { fn name(&self) -> &'static str; }
pub trait Advanced: Step + Named { type Output; }
pub struct Generic<T: Step> { pub inner: T }
impl<T: Step> Step for Generic<T> { fn step(&self, value: u32) -> u32 { self.inner.step(value) } }
pub struct Counter;
impl Step for Counter { fn step(&self, value: u32) -> u32 { value + 1 } }

pub fn concrete(value: u32) -> u32 { Counter.step(value) }
pub fn dynamic(value: &dyn Step) -> u32 { value.step(0) }
pub fn generic<T: Step>(value: &T) -> u32 { value.step(0) }
pub fn callback(value: fn(u32) -> u32) -> u32 { value(1) }
pub fn cross_file(value: u32) -> u32 { worker::run(value) }
pub fn fallible(value: Option<u32>) -> Option<u32> { Some(value? + 1) }
pub async fn asynchronous(value: u32) -> u32 { async { concrete(value) }.await }

pub fn infinite() -> ! { loop { core::hint::spin_loop(); } }
pub fn literal_question() -> &'static str { "a question? Unicode π" }
