from pydantic import BaseModel, ConfigDict
from scouter.genai import Agent, Prompt, Provider
from scouter.genai.google import GeminiSettings, GenerationConfig, ThinkingConfig

# take user input and reformulate it into a question for the LLM
reformulation_template = """
You are an expert in software development and debugging. Your task is to reformulate user input into a clear and concise question that can be answered by a large language model (LLM).
The user will provide a statement or a piece of code, and you need to transform it into a question that is specific, focused, and relevant to the context of the input.
Here are some examples of how to reformulate user input into questions:
1. User input: "I am trying to understand how to use the `map` function in Python."
   Reformulated question: "How do I use the `map` function in Python?"
2. User input: "I have a piece of code that is not working as expected."
   Reformulated question: "What could be the reason for my code not working as expected?"
3. User input: "Can you explain the concept of recursion?"
   Reformulated question: "What is recursion and how does it work in programming?"
4. User input: "I am getting an error when I try to run my script."
   Reformulated question: "What could be the cause of the error I am encountering when running my script?"
5. User input: "I want to optimize my code for better performance."
   Reformulated question: "What are some techniques to optimize code for better performance?"
Please reformulate the following user input into a question that can be answered by a large language model (LLM):
${user_input}
"""


# take a reformulated question and generate a response using the LLM
response_template = """
You are an expert in software development and debugging. Your task is to provide a detailed and informative response to the following question.
The question is based on user input that has been reformulated to be specific and focused. Your response should be clear, concise, and relevant to the context of the question.
Here are some examples of how to respond to questions:
1. Question: "How do I use the `map` function in Python?"
   Response: "The `map` function in Python applies a given function to all items in an iterable (like a list) and returns a map object (which is an iterator). You can convert it to a list using `list(map(function, iterable))`. For example, `list(map(lambda x: x * 2, [1, 2, 3]))` will return `[2, 4, 6]`."
2. Question: "What could be the reason for my code not working as expected?"
   Response: "There could be several reasons for your code not working as expected. Common issues include syntax errors, logical errors, or runtime exceptions. It would be helpful to see the specific error message or the part of the code that is causing the issue to provide a more accurate diagnosis."
3. Question: "What is recursion and how does it work in programming?"               
    Response: "Recursion is a programming technique where a function calls itself in order to solve a problem. It is often used to break down complex problems into simpler subproblems. A recursive function typically has a base case that stops the recursion and a recursive case that continues the recursion. For example, calculating the factorial of a number can be done using recursion."
4. Question: "What could be the cause of the error I am encountering when running my script?"
   Response: "The cause of the error when running your script could be due to various reasons such as missing dependencies, incorrect file paths, or syntax errors in the code. It would be helpful to see the specific error message you are encountering to provide a more accurate diagnosis."
5. Question: "What are some techniques to optimize code for better performance?"
   Response: "Some techniques to optimize code for better performance include using efficient algorithms and data structures, minimizing memory usage, avoiding unnecessary computations, and leveraging parallel processing when applicable. Profiling your code to identify bottlenecks can also help in optimizing performance."
Please provide a detailed and informative response to the following question:
${reformulated_query}
"""


class PromptState(BaseModel):
    model_config = ConfigDict(arbitrary_types_allowed=True)

    reformulated_prompt: Prompt = Prompt(
        message=reformulation_template,
        provider="gemini",
        model="gemini-2.5-flash-lite-preview-06-17",
        model_settings=GeminiSettings(
            generation_config=GenerationConfig(
                max_output_tokens=1024,
                thinking_config=ThinkingConfig(thinking_budget=0),
            ),
        ),
    )
    response_prompt: Prompt = Prompt(
        message=response_template,
        provider="gemini",
        model="gemini-2.5-flash-lite-preview-06-17",
        model_settings=GeminiSettings(
            generation_config=GenerationConfig(max_output_tokens=1024, temperature=0.2),
        ),
    )
    agent: Agent = Agent(
        provider=Provider.Gemini,
        system_instruction="You are an expert in software development and debugging.",
    )


prompt_state = PromptState()
