int work(int);
int completed_switch(int n) {
  switch(work(n)) {
  case 0: work(n); break;
  default: work(n+1); break;
  }
  return n;
}
int unfinished_switch(int n) {
  work(n);
  switch(work(n+1)) {
  case 0: if(n) work(n); break;
  case 1:
    switch(n) {
    case 2: work(n+2); break;
    default: work(n+3); break;
    }
    work(n+4);
    break;
  default: work(n+5); break;
  int nested_child(int x) { return work(x); }
  return n;
