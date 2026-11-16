// Runs `work` again when it throws, up to `times` tries, waiting `ms` between them.
async function retry(work, times = 3, ms = 100) {
  let last;
  for (let tried = 0; tried < times; tried += 1) {
    try {
      return await work();
    } catch (error) {
      last = error;
      await new Promise((done) => setTimeout(done, ms));
    }
  }
  throw last;
}

module.exports = { retry };
