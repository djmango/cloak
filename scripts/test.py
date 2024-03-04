from openai import OpenAI
import logging

# logging.basicConfig(level=logging.DEBUG)
# Set debug for all loggers
# logging.getLogger("openai").setLevel(logging.DEBUG)

client = OpenAI(base_url="http://localhost:8000/oai/v1", api_key="yo")


# Example of printing a stream of data, assuming 'your-engine-id' is valid for your local setup
def print_stream():
    stream = client.chat.completions.create(
        model="gpt-3.5-turbo",
        messages=[
            {"role": "system", "content": "You are a helpful assistant."},
            {"role": "user", "content": "What is the capital of the United States?"},
            {
                "role": "assistant",
                "content": "The capital of the United States is Washington, D.C.",
            },
            {"role": "user", "content": "What is the capital of France?"},
        ],
        max_tokens=50,
        stream=True,
    )

    print("Stream of data:")

    for chunk in stream:
        # print(chunk)
        print(chunk.choices[0].delta.content)
        # print(chunk.choices[0].delta.content or "", end="")


# Call the function to print the stream
print_stream()
