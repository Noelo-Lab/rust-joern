int walk_suffix(char *text, int stop) {
  char *cursor;
  char seen;
  cursor = text;
  seen = *text;
  do {
    seen = cursor[1];
    cursor = &cursor[1];
  } while (seen != stop);
  return 0;
}
void shift_window(long *window, long count) {
  long *head;
  long last;
  head = window;
  last = 0;
  while (count) {
    last = head[1];
    head[0] = last;
    head = &head[1];
    count = count - 1;
  }
  window[0] = last;
}
void fold_pairs(int *values, int *limit, int total) {
  int *left;
  int *right;
  left = values;
  right = limit;
  for (total = 0; left < right; total = total + *left) {
    right = &right[-1];
    *left = *right;
    left = &left[1];
  }
  *values = left[-1] + total;
}
