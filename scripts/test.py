import requests
import uuid
import os
from concurrent.futures import ThreadPoolExecutor, as_completed
import tiktoken

def add_memory(base_url, prompt, id=None):
    endpoint = f"{base_url}/add_memory_prompt"
    # Prepare the payload
    print("adding memory")
    print("== prompt ==")
    print(prompt)
    payload = {
        "prompt": prompt
    }
    if id:
        payload["id"] = str(id)

    headers = {
        "Content-Type": "application/json"
    }
    try:
        response = requests.post(endpoint, json=payload, headers=headers)
        response.raise_for_status()  # Raises an HTTPError for bad responses (4xx or 5xx)
        print(f"Status Code: {response.status_code}")
        print("Response:")
        print(response.text)
        return response.json()  # Return the parsed JSON response
    except requests.exceptions.RequestException as e:
        print(f"An error occurred: {e}")
        return None

def generate_from_chat(base_url, user_id, memory_prompt_id=None, max_samples=1000, samples_per_query=30):
    endpoint = f"{base_url}/generate_from_chat"
    # Prepare the payload
    print(f"== test for user {user_id} ==")
    payload = {
        "user_id": user_id
    }
    if memory_prompt_id:
        payload["memory_prompt_id"] = str(memory_prompt_id)
    else:
        # Generate a random UUID if not provided
        payload["memory_prompt_id"] = str(uuid.uuid4())
    
    if max_samples is not None:
        payload["max_samples"] = max_samples
    
    if samples_per_query is not None:
        payload["samples_per_query"] = samples_per_query

    headers = {
        "Content-Type": "application/json"
    }
    try:
        response = requests.post(endpoint, json=payload, headers=headers)
        response.raise_for_status()  # Raises an HTTPError for bad responses (4xx or 5xx)
        print(f"Status Code: {response.status_code}")
        print("Response:")
        print(response.text)
        return response
    except requests.exceptions.RequestException as e:
        print(f"An error occurred: {e}")
        return None

def count_memory_tokens(memories):
    # Initialize the tokenizer
    enc = tiktoken.get_encoding("cl100k_base")
    
    # Count tokens for each memory's content
    total_tokens = sum(len(enc.encode(memory["content"])) for memory in memories)
    
    return total_tokens

# Example usage
# TODO
'''
- run the new prompt on users responsible for top 20% traffic
- log it all
- read it and determine if memory useful or not
- use chats & memories to create few-shot examples
- iterate

'''
if __name__ == '__main__':
    cwd = os.path.dirname(os.path.realpath(__file__))
    print(cwd)
    base_url = "http://localhost:8000/memory"
                
    user_ids = ["user_01HRBJ8FVP3JT28DEWXN6JPKF5", # sully                                                    
                "user_01HY5EW9Z5XVE34GZXKH4NC2Y1", # minjune
                "user_01HZEP4TFR49AG913DPQJ6MASW",# some russian guy
                # https://us.posthog.com/project/59909/insights/K3EgabUz
                "user_01HZEP4TFR49AG913DPQJ6MASW", #1326 msgs https://us.posthog.com/project/59909/person/larinvasyl%40pm.me
                "user_01HVR20FDCZH3QX8WPYHR45MX7", #479 msgs https://us.posthog.com/project/59909/person/siuchun038%40gmail.com
                "user_01J13W84Z1TDYH5BEW4288KZQX", #385 msgs https://us.posthog.com/project/59909/person/4A6AAC56-382C-4BA1-8D99-19ECC76553BE
                "user_01J0JKNKX3FPMZ9JJ9NVG8D12A", # 291 msgs https://us.posthog.com/project/59909/person/alex.zorychta%40gmail.com
                "user_01J15P3MRBT039MTC0KWGD114R", # 240 msgs https://us.posthog.com/project/59909/person/ai%40hmphu.com
                "user_01J03D570TSXTNZ3FJGZFZ8VHA", # 208 msgs https://us.posthog.com/project/59909/person/F8037A10-280A-4ABA-9BB4-A4180E790BD3
                ] 

    user_ids = user_ids[:2]
    memory_prompt_id = '99ffa713-b445-40b8-a74c-ad8c1aee2675'
    for p in os.listdir(os.path.join(cwd, 'prompts')):
        pf = os.path.join(cwd, 'prompts', p)
        with open(pf, 'r') as f:
            prompt = f.read()
   #         add_memory(base_url, prompt)

    # Use ThreadPoolExecutor to run generate_from_chat concurrently
    with ThreadPoolExecutor(max_workers=10) as executor:
        futures = [executor.submit(generate_from_chat, base_url, user_id, memory_prompt_id) for user_id in user_ids]
        
        for future in as_completed(futures):
            try:
                response = future.result()
                if response:
                    memories = response.json()
                    total_tokens = count_memory_tokens(memories)
                    print(f"Total tokens in generated memories: {total_tokens}")
                    
                    for memory in memories:
                        log_file_path = os.path.join(cwd, 'logs', f'{memory_prompt_id}-{memory["user_id"]}.csv')
                        if os.path.exists(log_file_path):
                            os.makedirs(os.path.dirname(os.path.join (cwd, 'logs')), exist_ok=True)
                        with open(log_file_path, 'a') as log_file:
                            log_file.write(f'{memory["id"]},{memory["content"]}\n')
            except Exception as exc:
                print(f'Generated an exception: {exc}')