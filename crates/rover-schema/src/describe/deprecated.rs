use apollo_compiler::{
    ast::{DirectiveList, InputValueDefinition, Value},
    schema::{
        Component, EnumType, EnumValueDefinition, FieldDefinition, InterfaceType, ObjectType,
    },
};

pub(super) trait IsDeprecated {
    fn is_deprecated(&self) -> bool;
    fn deprecation_reason(&self) -> Option<String>;
}

impl IsDeprecated for DirectiveList {
    fn is_deprecated(&self) -> bool {
        self.get("deprecated").is_some()
    }

    fn deprecation_reason(&self) -> Option<String> {
        self.get("deprecated").and_then(|d| {
            d.arguments
                .iter()
                .find(|arg| arg.name == "reason")
                .and_then(|arg| {
                    if let Value::String(s) = &*arg.value {
                        Some(s.to_string())
                    } else {
                        None
                    }
                })
        })
    }
}

impl IsDeprecated for Component<FieldDefinition> {
    fn is_deprecated(&self) -> bool {
        self.directives.is_deprecated()
    }

    fn deprecation_reason(&self) -> Option<String> {
        self.directives.deprecation_reason()
    }
}

impl IsDeprecated for FieldDefinition {
    fn is_deprecated(&self) -> bool {
        self.directives.is_deprecated()
    }

    fn deprecation_reason(&self) -> Option<String> {
        self.directives.deprecation_reason()
    }
}

impl IsDeprecated for Component<EnumValueDefinition> {
    fn is_deprecated(&self) -> bool {
        self.directives.is_deprecated()
    }

    fn deprecation_reason(&self) -> Option<String> {
        self.directives.deprecation_reason()
    }
}

impl IsDeprecated for &InputValueDefinition {
    fn is_deprecated(&self) -> bool {
        self.directives.is_deprecated()
    }

    fn deprecation_reason(&self) -> Option<String> {
        self.directives.deprecation_reason()
    }
}

pub(super) trait DeprecatedFields {
    fn deprecated_fields(&self) -> Vec<&FieldDefinition>;
}

impl DeprecatedFields for ObjectType {
    fn deprecated_fields(&self) -> Vec<&FieldDefinition> {
        self.fields
            .values()
            .filter(|f| f.is_deprecated())
            .map(|f| f.as_ref())
            .collect()
    }
}

impl DeprecatedFields for InterfaceType {
    fn deprecated_fields(&self) -> Vec<&FieldDefinition> {
        self.fields
            .values()
            .filter(|f| f.is_deprecated())
            .map(|f| f.as_ref())
            .collect()
    }
}

pub(super) trait DeprecatedValues {
    fn deprecated_values(&self) -> Vec<&EnumValueDefinition>;
}

impl DeprecatedValues for EnumType {
    fn deprecated_values(&self) -> Vec<&EnumValueDefinition> {
        self.values
            .values()
            .filter(|v| v.is_deprecated())
            .map(|v| v.as_ref())
            .collect()
    }
}
