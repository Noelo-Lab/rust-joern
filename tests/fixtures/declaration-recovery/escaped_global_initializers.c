int before(int n) { return n; }
extern const wchar16 const wide_values[2] = {\"K\", \"M\"};
int after_wide(int n) { return n; }
extern const char narrow_values[2] = {\"K\", \"M\"};
int after_narrow(int n) { return n; }
extern const Pair dict_values[1] = {{\'offset\': 123, \'info\': 23}};
int after_dict(int n) { return n; }
extern const char *valid_values[2] = {"\"K\"", "\"M\""};
int after_valid(int n) { return n; }
extern const char *multiline_valid[2] = {
    "K", "M"
};
int after_multiline(int n) { return n; }
