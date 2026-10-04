#![allow(dead_code)]
pub mod worker;

pub trait Step { fn step(&self, value: u32) -> u32; }
pub trait Named { fn name(&self) -> &'static str; }
pub trait Advanced: Step + Named { type Output; }
pub struct Generic<T: Step> { pub inner: T }
impl<T: Step> Step for Generic<T> { fn step(&self, value: u32) -> u32 { self.inner.step(value) } }
#[derive(Debug)]
#[doc = "Explicit domain documentation π"]
pub struct Counter;
#[cfg(any())]
#[derive(Debug)]
pub struct ExcludedCounter;
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
#[allow(clippy::needless_return)]
pub fn recursive(value: u32) -> u32 {
    if value == 0 { 0 } else { recursive(value - 1) }
}

#[allow(dead_code)]
pub mod inline {
    #[derive(Debug)]
    pub struct Inner;
    pub mod nested { pub fn run() {} }
}

macro_rules! generate_function { () => { pub fn generated() -> u32 { 5 } }; }
generate_function!();

#[cfg(annotation_fixture)]
#[derive(Debug)]
pub struct ConfigSelected;
