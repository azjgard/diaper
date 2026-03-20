// Sample file with a known violation for E2E testing.
async function handleRequest() {
  const data = await fetch("/api");
  return data;
}

export default handleRequest;
