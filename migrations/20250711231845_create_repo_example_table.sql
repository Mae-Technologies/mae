
SELECT app.create_table_from_spec(
'{
  "table_name": "repoexample",
  "columns": [
    { "name": "string_value", "type": "text"},
    { "name": "value", "type": "int4"}
  ]
}
  '::jsonb);
