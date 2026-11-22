struct Gauge {
  Gauge();
  explicit Gauge(int level);
  void store(int *low, int *high) const;
  int peek(int scale) const;
  Gauge *operator->();
};

int dot_member(int level, int mode) {
  int low = 0, high = 0;
  if (mode == 1) {
    Gauge(level).store(&low, &high);
  } else {
    low = mode;
  }
  return low + high;
}

int arrow_member(int level, int mode) {
  int total = mode;
  if (mode > 2) {
    Gauge(level)->peek(total);
  } else {
    total = 1;
  }
  return total;
}

int grouped_declaration(int mode) {
  Gauge (gauge);
  if (mode != 0) {
    return gauge.peek(mode);
  }
  return 0;
}
