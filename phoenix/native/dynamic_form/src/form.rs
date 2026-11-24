//! `DynamicForm.Renderer.LiveComponent`'s lifecycle: the form mounted
//! from initial data with its defaults, changed and submitted with the
//! browser's params, and entries added and removed. Where the component
//! keeps its changeset in the socket, this hands the page a `FormState`
//! to send back with the next event, so the Rust keeps nothing between
//! calls.

use ecto_changeset::{Params, form};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::changeset::{FormChangeset, changeset};
use crate::definition::Definition;
use crate::nested::{find_question, new_entry, seed_entries, seed_entry_ids, update_entry_list};
use crate::view::{FormTarget, FormView, view};

/// What the form holds between events: the params its changeset was built
/// from, and whether a submit has failed, from which errors show.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FormState {
    pub params: Params,
    pub submitted: bool,
}

/// The form as shown, and the state to send back with the next event.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Shown {
    #[serde(flatten)]
    pub view: FormView,
    pub state: FormState,
}

/// A form: its definition, its `<form>` element's id, the name its params
/// are under, and the data it starts from, a stored record's for editing.
#[derive(Debug, Clone)]
pub struct DynamicForm<'a> {
    pub definition: &'a Definition,
    pub id: &'a str,
    pub name: &'a str,
    pub data: Params,
}

impl<'a> DynamicForm<'a> {
    pub fn new(definition: &'a Definition, id: &'a str, name: &'a str) -> Self {
        DynamicForm {
            definition,
            id,
            name,
            data: Params::new(),
        }
    }

    pub fn with_data(mut self, data: Params) -> Self {
        self.data = data;
        self
    }

    /// The form as it first shows: the data with each question's
    /// `defaultValue` where it has none, nested forms seeded with their
    /// `panelCount` entries, and each entry given an id.
    pub fn mount(&self) -> Shown {
        let mut params = self.initial_data();
        seed_entry_ids(&mut params, &self.definition.questions());
        self.shown(&params, false)
    }

    /// The form after the browser's `change`, given as its `FormData`
    /// entries.
    pub fn change(&self, state: &FormState, entries: &[(String, String)]) -> Result<Shown, String> {
        Ok(self.change_params(state, &self.form_params(entries)?))
    }

    /// The form after a change, given as the params under its name.
    pub fn change_params(&self, state: &FormState, form_params: &Params) -> Shown {
        let params = self.merge(state, form_params);
        self.shown(&params, state.submitted)
    }

    /// The form submitted: `Ok` with the form's data when it is valid, as
    /// the library's payload has it, or the form with its errors showing.
    pub fn submit(
        &self,
        state: &FormState,
        entries: &[(String, String)],
    ) -> Result<Result<Map<String, Value>, Shown>, String> {
        Ok(self.submit_params(state, &self.form_params(entries)?))
    }

    /// The form submitted, given as the params under its name.
    pub fn submit_params(
        &self,
        state: &FormState,
        form_params: &Params,
    ) -> Result<Map<String, Value>, Shown> {
        let params = self.merge(state, form_params);
        let form = changeset(&self.definition.elements, &params);
        if form.valid() {
            Ok(form.data())
        } else {
            Err(self.view_of(&form, true))
        }
    }

    /// The form with a new entry at the end of the nested form at `path`,
    /// `"rooms"` or `"floors.0.rooms"`.
    pub fn add_entry(&self, state: &FormState, path: &str) -> Result<Shown, String> {
        let segments: Vec<&str> = path.split('.').collect();
        let question = find_question(&self.definition.elements, &segments)
            .ok_or_else(|| format!("the form has no nested form at {path}"))?;
        let seed = new_entry(question);
        self.update_entries(state, &segments, &move |mut entries| {
            entries.push(seed.clone());
            entries
        })
    }

    /// The form without the entry at `index` of the nested form at `path`.
    pub fn remove_entry(
        &self,
        state: &FormState,
        path: &str,
        index: usize,
    ) -> Result<Shown, String> {
        let segments: Vec<&str> = path.split('.').collect();
        self.update_entries(state, &segments, &move |mut entries| {
            if index < entries.len() {
                entries.remove(index);
            }
            entries
        })
    }

    fn update_entries(
        &self,
        state: &FormState,
        path: &[&str],
        update: &dyn Fn(Vec<Value>) -> Vec<Value>,
    ) -> Result<Shown, String> {
        let mut params = Value::Object(state.params.clone());
        update_entry_list(&mut params, path, update)?;
        let Value::Object(mut params) = params else {
            unreachable!("the params stay a map");
        };
        seed_entry_ids(&mut params, &self.definition.questions());
        Ok(self.shown(&params, state.submitted))
    }

    /// `apply_default_values/2`.
    fn initial_data(&self) -> Params {
        let mut params = self.data.clone();
        let questions = self.definition.questions();
        for question in &questions {
            if let Some(default) = &question.default_value {
                params
                    .entry(question.name.clone())
                    .or_insert_with(|| default.clone());
            }
        }
        seed_entries(&mut params, &questions);
        params
    }

    /// `merge_data/2`: the initial data, under what the form holds, under
    /// what the browser sent, so values the browser doesn't send, like a
    /// hidden section's, keep what the user entered.
    fn merge(&self, state: &FormState, form_params: &Params) -> Params {
        let mut params = self.initial_data();
        params.extend(state.params.clone());
        params.extend(form_params.clone());
        params
    }

    /// The params under the form's name in its `FormData` entries,
    /// decoded as Plug decodes them.
    fn form_params(&self, entries: &[(String, String)]) -> Result<Params, String> {
        let mut params = form::decode(
            entries
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str())),
        )?;
        match params.remove(self.name) {
            Some(Value::Object(params)) => Ok(params),
            _ => Ok(Params::new()),
        }
    }

    fn shown(&self, params: &Params, submitted: bool) -> Shown {
        let form = changeset(&self.definition.elements, params);
        self.view_of(&form, submitted)
    }

    fn view_of(&self, form: &FormChangeset, submitted: bool) -> Shown {
        let target = FormTarget {
            id: self.id,
            name: self.name,
            submitted,
        };
        Shown {
            view: view(self.definition, target, form),
            state: FormState {
                params: form.changeset.params().clone(),
                submitted,
            },
        }
    }
}
