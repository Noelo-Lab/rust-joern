int qualified_definition(NS::T value) { return 1; }
int after_qualified_definition(int x) { return x; }
int qualified_prototype(NS::T value);
int after_qualified_prototype(int x) { return x; }
int dollar_definition(NS::$123 value) { return 2; }
int after_dollar_definition(int x) { return x; }
int dollar_prototype(NS::$123 value);
int after_dollar_prototype(int x) { return x; }
