from collections import defaultdict
import json

def format_memories(input_json):
    memories = json.loads(input_json)
    grouped_memories = defaultdict(list)
    
    for memory in memories:
        grouped_memories[memory['grouping']].append(memory['content'])
    
    formatted_output = []
    for group, contents in grouped_memories.items():
        formatted_output.append(f"<memory>\n{group}")
        for content in contents:
            formatted_output.append(f"- {content}")
        formatted_output.append("</memory>")
    
    return "\n".join(formatted_output)

if '__main__' == __name__:
    input_json = ''  # Your input JSON string
    formatted_output = format_memories(input_json)
    print(formatted_output)