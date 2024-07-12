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
    input_json = '[{"id":"ba0ed310-a19b-4995-a939-9269777bae8e","user_id":"user_01J0HEK8WZ624C489TZJD5D1X8","content":"Has an informal communication style but is actively working towards more professional and clear business communications.","created_at":"2024-07-11T17:50:56.779556Z","updated_at":"2024-07-12T01:57:22.590230Z","deleted_at":null,"grouping":"Communication"},{"id":"2e332fce-79a2-4d30-aa9d-07b2531844ee","user_id":"user_01J0HEK8WZ624C489TZJD5D1X8","content":"Prefers to communicate in Portuguese","created_at":"2024-07-12T01:57:22.670363Z","updated_at":"2024-07-12T01:57:22.670377Z","deleted_at":null,"grouping":"Communication"},{"id":"166d4e9a-9a56-4390-ad71-b599fac0493c","user_id":"user_01J0HEK8WZ624C489TZJD5D1X8","content":"Communicates in a brief, direct style, often using lowercase without punctuation","created_at":"2024-07-12T01:57:22.670508Z","updated_at":"2024-07-12T01:57:22.670511Z","deleted_at":null,"grouping":"Communication"},{"id":"1204cb68-f28d-4cec-b5d9-cd6f1e837329","user_id":"user_01J0HEK8WZ624C489TZJD5D1X8","content":"Interested in technical topics, particularly Apple shortcuts and productivity tools","created_at":"2024-07-12T01:57:22.670563Z","updated_at":"2024-07-12T01:57:22.670565Z","deleted_at":null,"grouping":"Interests"},{"id":"ebe776ee-1a53-4ea6-9a0d-1372e0205902","user_id":"user_01J0HEK8WZ624C489TZJD5D1X8","content":"Business owner/manager undergoing company restructuring and acquisition","created_at":"2024-07-12T01:57:22.670611Z","updated_at":"2024-07-12T01:57:22.670613Z","deleted_at":null,"grouping":"Work"},{"id":"c125be49-66a9-4149-9734-2bff9c0fddba","user_id":"user_01J0HEK8WZ624C489TZJD5D1X8","content":"Dealing with project delays","created_at":"2024-07-12T01:57:22.670655Z","updated_at":"2024-07-12T01:57:22.670656Z","deleted_at":null,"grouping":"Work"},{"id":"815b0739-d602-428c-86c1-60d8641472a3","user_id":"user_01J0HEK8WZ624C489TZJD5D1X8","content":"Seeking to improve time management and client relations","created_at":"2024-07-12T01:57:22.670702Z","updated_at":"2024-07-12T01:57:22.670703Z","deleted_at":null,"grouping":"Goals"}]'  # Your input JSON string
    formatted_output = format_memories(input_json)
    print(formatted_output)