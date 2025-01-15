use crate::FeatureMap;
use scouter_error::ScouterError;

pub trait Config {

    fn update_feature_map(&mut self, feature_map: FeatureMap)-> Result<(), ScouterError>;
}

pub trait Profile {
    fn get_feature_map(&self) -> Option<FeatureMap>;
}

