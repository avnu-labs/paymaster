use std::hash::RandomState;

use indexmap::IndexMap;
use starknet::core::types::typed_data::{FieldDefinition, FullTypeReference, Revision, StructDefinition, TypeDefinition, Types};

#[derive(Default)]
pub struct TypeBuilder {
    definitions: IndexMap<String, TypeDefinition, RandomState>,
}

impl TypeBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_definition(&'_ mut self, name: &str) -> StructDefinitionBuilder<'_> {
        StructDefinitionBuilder {
            builder: self,

            name: name.to_string(),
            fields: vec![],
        }
    }

    pub fn build(self, revision: Revision) -> Types {
        Types::new(revision, self.definitions)
    }
}

pub struct StructDefinitionBuilder<'a> {
    builder: &'a mut TypeBuilder,

    name: String,
    fields: Vec<FieldDefinition>,
}

impl<'a> StructDefinitionBuilder<'a> {
    pub fn add_field(mut self, field: &str, r#type: FullTypeReference) -> Self {
        self.fields.push(FieldDefinition::new(field.to_string(), r#type));

        self
    }

    pub fn register(self) {
        self.builder
            .definitions
            .insert(self.name, TypeDefinition::Struct(StructDefinition { fields: self.fields }));
    }
}
