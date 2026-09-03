use serde::Deserialize;
use std::collections::HashMap;

#[allow(dead_code)]
pub type Env = super::fixture_types::Env;
#[allow(dead_code)]
pub type AccountState = super::fixture_types::AccountFixture;
#[allow(dead_code)]
pub type TestTransaction = super::fixture_types::TxFixture;
#[allow(dead_code)]
pub type PostExpectation = super::fixture_types::PostStateExpectation;
#[allow(dead_code)]
pub type StateTest = super::fixture_types::StateTestCase;

#[derive(Debug, Deserialize)]
pub struct TestSuite(pub HashMap<String, StateTest>);
