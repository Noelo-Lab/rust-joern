int before(int n) { return n; }
extern const char bad_values[2] = {\"K\", \"M\"};
int swallowed_nested(int n) { if (n) { n++; } else { n--; } return n; }
int after_nested(int n) { return n; }
extern const Pair bad_dict[1] = {{\'offset\': 1, \'info\': 2}};
int swallowed_initializer(int n) { int values[2] = {n, n+1}; while(n) { n--; } return values[0]; }
int after_initializer(int n) { return n; }
extern const char bad_quote[1] = {"broken
int swallowed_quote(int n) { if (n) { return n; } return 0; }
int after_quote(int n) { return n; }
extern const char *valid_values[2] = {"\"K\"", "\"M\""};
int after_valid(int n) { return n; }
