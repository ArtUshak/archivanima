use validator::ValidationErrors;

pub trait IdSet {
    fn is_valid_id(&self, id: &str) -> bool;
    fn get_option_list(&self) -> Vec<(String, String)>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdField {
    pub value: Option<String>,
    pub values: Vec<(String, String)>,
}

impl IdField {
    pub fn load<S: IdSet>(value: Option<String>, id_set: &S) -> (Self, bool) {
        match value {
            None => (
                (IdField {
                    value: None,
                    values: id_set.get_option_list(),
                }),
                false,
            ),
            Some(value_real) if id_set.is_valid_id(&value_real) => (
                (IdField {
                    value: Some(value_real),
                    values: id_set.get_option_list(),
                }),
                false,
            ),
            Some(_) => (
                (IdField {
                    value: None,
                    values: id_set.get_option_list(),
                }),
                true,
            ),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExtraValidatedForm<T>(pub T, pub ValidationErrors);
