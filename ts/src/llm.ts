/** Trait for LLM completion providers. */
export interface LlmProvider {
  complete(prompt: string): string | Promise<string>;
}

/**
 * A mock LLM provider for testing. Returns canned responses based on prompt content.
 * The first needle found in the prompt determines the response.
 * If no needle matches, `defaultResponse` is returned.
 */
export class MockProvider implements LlmProvider {
  responses: Array<[string, string]> = [];
  defaultResponse: string;

  constructor(defaultResponse: string) {
    this.defaultResponse = defaultResponse;
  }

  withResponse(needle: string, response: string): this {
    this.responses.push([needle, response]);
    return this;
  }

  complete(prompt: string): string {
    for (const [needle, response] of this.responses) {
      if (prompt.includes(needle)) return response;
    }
    return this.defaultResponse;
  }
}
