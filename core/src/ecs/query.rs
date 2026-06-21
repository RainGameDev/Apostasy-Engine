#[derive(Clone, Copy, PartialEq, Eq)]
pub enum QueryType {
    Include,
    Exclude,
}

#[derive(Clone, PartialEq, Eq)]
pub struct QueryComponent {
    pub component: String,
    pub query_typer: QueryType,
}

pub struct Query {
    pub query_components: Vec<QueryComponent>,
}
