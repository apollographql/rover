use rover_studio::types::GraphRef;

use crate::RoverClientError;

/// Reports the `graph { variant { ... } }` two-level lookup.
///Callers flatten the generated response type's `Option<Option<V>>`
/// (e.g. `graph.and_then(|g| g.variant)`) before calling this.
#[allow(dead_code)]
pub(crate) fn require_variant<V>(
    variant: Option<V>,
    graph_ref: &GraphRef,
) -> Result<V, RoverClientError> {
    variant.ok_or_else(|| RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;

    #[test]
    fn require_variant_resolves_present_variant() {
        let graph_ref: GraphRef = "test-graph@test-variant".parse().unwrap();
        assert_that!(require_variant(Some(42), &graph_ref))
            .is_ok()
            .is_equal_to(42);
    }

    #[test]
    fn require_variant_errors_on_missing_variant() {
        let graph_ref: GraphRef = "test-graph@test-variant".parse().unwrap();
        let err = require_variant::<i32>(None, &graph_ref).unwrap_err();
        let RoverClientError::GraphNotFound {
            graph_ref: actual_graph_ref,
        } = err
        else {
            panic!("expected RoverClientError::GraphNotFound, got {err:?}");
        };
        assert_that!(&actual_graph_ref).is_equal_to(&graph_ref);
    }
}
