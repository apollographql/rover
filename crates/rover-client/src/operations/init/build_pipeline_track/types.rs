use super::runner::build_pipeline_track_mutation;

pub(crate) type ResponseData = build_pipeline_track_mutation::ResponseData;
pub(crate) type MutationVariables = build_pipeline_track_mutation::Variables;

pub struct BuildPipelineTrackInput {
    pub graph_id: String,
    pub version: build_pipeline_track_mutation::BuildPipelineTrack,
    pub variant_name: String,
}

impl From<BuildPipelineTrackInput> for MutationVariables {
    fn from(input: BuildPipelineTrackInput) -> Self {
        Self {
            graph_id: input.graph_id,
            version: input.version,
            name: input.variant_name,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildPipelineTrackResponse {
    pub id: String,
    pub federation_version: String,
}

impl From<build_pipeline_track_mutation::BuildPipelineTrackMutationGraphVariantUpdateVariantFederationVersion>
    for BuildPipelineTrackResponse
{
    fn from(build_pipeline_track: build_pipeline_track_mutation::BuildPipelineTrackMutationGraphVariantUpdateVariantFederationVersion) -> Self {
        Self {
            id: build_pipeline_track.id,
            federation_version: build_pipeline_track.federation_version.unwrap(),
        }
    }
}
