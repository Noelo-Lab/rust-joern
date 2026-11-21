long micros(long dt) {
  if (dt < 0)
    dt = 0;
  return chrono::duration_cast<chrono::microseconds>(
             std::chrono::steady_clock::duration{ dt })
      .count();
}

long wrap(long v, int n) {
  long total = 0;
  while (n-- > 0)
    total += measure(sample{ v, n });
  holder = ns::box{ total };
  return total;
}
