namespace demo {
int tick(int);
struct Base { explicit Base(int); };
struct Counter : Base {
  int count;
  int *cursor;
  Counter(int start, int *at);
};
Counter::Counter(int start, int *at) : Base(tick(start)), count(tick(start)), cursor(at) {
  while (count > 0) count = tick(count);
  if (cursor) *cursor = count;
}
}
