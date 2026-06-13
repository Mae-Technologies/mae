pub struct ListQuery<F, S> {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
    pub filter: Option<F>,
    pub sort: Option<Vec<S>>
}
