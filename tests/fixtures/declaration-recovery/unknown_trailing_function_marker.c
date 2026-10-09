int work(int);
int pure_suffix(int n) __pure { if(n) return work(n); return 0; }
int after_pure_suffix(int n) { return work(n); }
int arbitrary_suffix(int n) custom_marker { if(n) return work(n); return 0; }
int after_arbitrary_suffix(int n) { return work(n); }
int called_suffix(int n) custom_marker(n) { if(n) return work(n); return 0; }
int after_called_suffix(int n) { return work(n); }
int recognized_attribute(int n) __attribute__((pure)) { if(n) return work(n); return 0; }
int after_recognized_attribute(int n) { return work(n); }
