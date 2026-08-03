export async function handler(input) {
  return { message: `Hello, ${input.name || 'world'}!` };
}
