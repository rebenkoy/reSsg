use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use anyhow::anyhow;
use minijinja::{Error, State, Value};
use minijinja::value::{Enumerator, Object, ObjectRepr};
use serde::de::Error as _;
use rsfs::GenFS;
use crate::build::custom_functions::stateful::{RenderState, StatefulFunction};
use crate::build::renderer_state::{get_state, lock_state};


#[derive(Debug, Clone)]
struct Array {
    elements: Vec<Value>,
}

impl Array {
    fn new() -> Self {
        Self {
            elements: Vec::new(),
        }
    }
    fn push(&mut self, value: Value) {
        self.elements.push(value);
    }
}

impl Object for Array {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Seq
    }

    fn get_value(self: &Arc<Self>, key: &Value) -> Option<Value> {
        self.elements.get(key.as_usize()?).cloned()
    }

    fn enumerate(self: &Arc<Self>) -> Enumerator {
        Enumerator::Seq(self.elements.len())
    }
}

pub fn collected_array(state: &State, key: String) -> Result<Value, Error> {
    let renderer_state = get_state(state)?;
    let locked_state = lock_state(&renderer_state)?;
    let remvalue = &locked_state.remvalue;
    match remvalue.state {
        RenderState::FirstPass => {
            Ok(Value::from_object(Array::new()))
        }
        RenderState::LastPass => {
            if !remvalue.map.contains_key(&key) {
                Ok(Value::from_object(Array::new()))
            } else {
                Ok(Value::from_object(
                    remvalue
                        .map
                        .get(&key)
                        .ok_or(Error::custom(format!("Array `{}` is not found", key)))?
                        .clone()
                ))
            }
        }
    }

}

pub fn push_to_array(state: &State, key: String, value: Value) -> Result<Value, Error> {
    let renderer_state = get_state(state)?;
    let mut locked_state = lock_state(&renderer_state)?;
    let remvalue = &mut locked_state.remvalue;
    match remvalue.state {
        RenderState::FirstPass => {
            if !remvalue.map.contains_key(&key) {
                remvalue.map.insert(key.clone(), Array::new());
            }
            remvalue
                .map
                .get_mut(&key)
                .ok_or(Error::custom(format!("Array `{}` is not found", key)))?
                .push(value);

        }
        RenderState::LastPass => {}
    }
    Ok(Value::from_bytes(vec![]))
}


#[derive(Debug, Clone)]
pub struct RemValueState {
    map: HashMap<String, Array>,
    state: RenderState
}

impl Default for RemValueState {
    fn default() -> Self {
        Self {
            map: HashMap::new(),
            state: RenderState::FirstPass
        }
    }
}

impl StatefulFunction for RemValueState {
    fn build<FS: GenFS>(state: &State, fs: &mut FS) -> Result<Self, anyhow::Error>  {
        let renderer_state = get_state(state)?;
        let locked_state = lock_state(&renderer_state)?;
        let RemValueState { map, state } = &locked_state.remvalue;
        Ok(Self {
            map: map.clone(),
            state: RenderState::LastPass,
        })
    }
}