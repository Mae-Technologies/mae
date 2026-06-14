use crate::repo::implement::ToField;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ListQuery<Q, F: ToField> {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub filter: Q,
    pub sort_by: Option<Vec<F>>
}

impl ListQuery<Q, F> {
    fn new(filter: Q) -> anyhow::Result<Self> {
        Ok(Self { filter, offset: None, sort_by: None, limit: None })
    }
}
