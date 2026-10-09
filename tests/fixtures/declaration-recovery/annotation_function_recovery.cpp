int work(int);
void original(int n) __noreturn(int n) { if(n) work(n); work(0); }
int after(int n) { return work(n); }
void plain(int n) unknown_annotation(int n) { if(n) work(n); work(0); }
int after_plain(int n) { return work(n); }
void recognized(int n) __cdecl { if(n) work(n); work(0); }
int after_recognized(int n) { return work(n); }
void standard(int n) const(int n) { if(n) work(n); work(0); }
int after_standard(int n) { return work(n); }
