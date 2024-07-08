import requests
import uuid
import os
import json
import tiktoken

from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime, timedelta

def get_jwt_token():
    # put ur jwt here
    return 'eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyXzAxSFJCSjhGVlAzSlQyOERFV1hONkpQS0Y1IiwiZXhwIjoxNzIzNDM0NDUxLCJpYXQiOjE3MjA0MTA0NTF9.BUTV6emjv_sny-wc_rZpUzyskxCsfeNEV7bX2HeFbhI'

def delete_all_memories(base_url, user_id):
    endpoint = f"{base_url}/memories/delete_all"
    token = get_jwt_token()
    payload = {"user_id": user_id}
    headers = {
        "Content-Type": "application/json",
        "Authorization": f"Bearer {token}"
    }
    response = requests.post(endpoint, json=payload, headers=headers)
    response.raise_for_status()
    return response.json()

def add_memory(base_url, prompt, id=None):
    endpoint = f"{base_url}/memories/add_memory_prompt"
    # Prepare the payload
    token = get_jwt_token()
    print("adding memory")
    print("== prompt ==")
    print(prompt)
    payload = {
        "prompt": prompt
    }
    if id:
        payload["id"] = str(id)

    headers = {
        "Content-Type": "application/json",
        "Authorization": f"Bearer {token}"
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

# Modify this function to include the JWT token
def generate_from_chat(base_url, user_id, memory_prompt_id=None, max_samples=1000, samples_per_query=30, range=None):
    endpoint = f"{base_url}/memories/generate_from_chat"
    # Get JWT token
    token = get_jwt_token()
    
    # Prepare the payload
    print(f"== test for user {user_id} ==")
    payload = {
        "user_id": user_id
    }
    if memory_prompt_id:
        payload["memory_prompt_id"] = str(memory_prompt_id)
    else:
        payload["memory_prompt_id"] = str(uuid.uuid4())
    
    if max_samples is not None:
        payload["max_samples"] = max_samples
    
    if samples_per_query is not None:
        payload["samples_per_query"] = samples_per_query
    
    if range is not None:
        payload["range"] = range

    headers = {
        "Content-Type": "application/json",
        "Authorization": f"Bearer {token}"  # Add the JWT token to the headers
    }
    try:
        response = requests.post(endpoint, json=payload, headers=headers)
        response.raise_for_status()
        print(f"Status Code: {response.status_code}")
        print("Response:")
        print(response.text)
        return response
    except requests.exceptions.RequestException as e:
        print(f"An error occurred: {e}")
        return None

def count_memory_tokens(memories):
    enc = tiktoken.get_encoding("cl100k_base")
    total_tokens = sum(len(enc.encode(memory["content"])) for memory in memories)
    return total_tokens

def test_memory_increment(base_url, user_id, memory_prompt_id, days_back=10):
    now = datetime.utcnow()
    # Ensure logs directory exists
    log_dir = os.path.join(os.path.dirname(os.path.realpath(__file__)), 'logs', 'memory_experiments')
    os.makedirs(log_dir, exist_ok=True)
    
    # Test case 1: Generate from beginning of time to 'days_back' days ago
    start_date = datetime.min  # Beginning of time
    end_date = now - timedelta(days=days_back)
    range_payload = [0, int(end_date.timestamp())]
    response = generate_from_chat(base_url, user_id, memory_prompt_id, range=range_payload)
    print(f"Test case 1: {start_date} to {end_date} - {'Success' if response else 'Fail'}")
    log_response(f"beginning_to_{days_back}_days_ago", response, log_dir)
    
    # Test cases 2 to days_back+1: Generate between ranges
    for i in range(days_back):
        start_date = now - timedelta(days=days_back-i)
        end_date = now - timedelta(days=days_back-1-i)
        range_payload = [int(start_date.timestamp()), int(end_date.timestamp())]
        response = generate_from_chat(base_url, user_id, memory_prompt_id, range=range_payload)
        print(f"Test case {i+2}: {start_date} to {end_date} - {'Success' if response else 'Fail'}")
        log_response(f"range_{days_back-i}_{days_back-1-i}_days_ago", response, log_dir)
    
    # Test case days_back+2: Generate for yesterday to now
    yesterday = now - timedelta(days=1)
    range_payload = [int(yesterday.timestamp()), int(now.timestamp())]
    response = generate_from_chat(base_url, user_id, memory_prompt_id, range=range_payload)
    print(f"Test case {days_back+2}: {yesterday} to {now} - {'Success' if response else 'Fail'}")
    log_response("range_yesterday_to_now", response, log_dir)
    
    print("All test cases completed.")

def get_all(base_url, user_id, memory_prompt_id=None, format=False):
    endpoint = f"{base_url}/memories/get_all"
    token = get_jwt_token()
    params = {"user_id": user_id}  # Add user_id to the params
    if memory_prompt_id:
        params["memory_prompt_id"] = str(memory_prompt_id)
    if format:
        params["format"] = "true"
    
    headers = {
        "Authorization": f"Bearer {token}"
    }
    try:
        response = requests.get(endpoint, params=params, headers=headers)
        response.raise_for_status()
        result = response.json()
        # Write result to file
        log_dir = os.path.join(os.path.dirname(os.path.realpath(__file__)), 'logs', 'memory_experiments')
        os.makedirs(log_dir, exist_ok=True)
        file_path = os.path.join(log_dir, f"{user_id}_memory.txt")
        
        with open(file_path, 'w') as f:
            json.dump(result, f, indent=2)
        
        return result
    except requests.exceptions.RequestException as e:
        print(f"An error occurred: {e}")
        return None

def log_response(test_case, response, log_dir):
    log_file_path = os.path.join(log_dir, f'{test_case}.log')
    with open(log_file_path, 'w') as log_file:
        if response is None:
            log_file.write(f"Test case: {test_case}\nResponse: None\n")
        else:
            log_file.write(f"Test case: {test_case}\nStatus Code: {response.status_code}\n")
            memories = response.json()
            for memory in memories:
                log_file.write(f'{memory["content"]},{memory["group_id"]}\n')

if __name__ == '__main__':
    cwd = os.path.dirname(os.path.realpath(__file__))
    base_url = "http://localhost:8000"
    user_ids = ['user_01HRBJ8FVP3JT28DEWXN6JPKF5', 'user_01HY5EW9Z5XVE34GZXKH4NC2Y1']
    memory_prompt_id = 'bb324e6e-de12-4078-bd1c-ebc9d934b535'
    for user_id in user_ids:
        delete_all_memories(base_url, user_id)
        test_memory_increment(base_url, user_id, memory_prompt_id)
        print(f'getting mem for {user_id}')
        final_mem = get_all(base_url, user_id, format=True)
        print(final_mem)
    exit(-1)

    cwd = os.path.dirname(os.path.realpath(__file__))
    print(cwd)
    base_url = "http://localhost:8000"
                
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

    user_ids = [user_ids[1]]
    memory_prompt_id = 'b66ebb74-09c2-4c67-bf99-52c05e7dbe44'
    for p in os.listdir(os.path.join(cwd, 'prompts')):
        pf = os.path.join(cwd, 'prompts', p)
        with open(pf, 'r') as f:
            prompt = f.read()
            #add_memory(base_url, prompt)

    delete_all_memories(base_url, user_ids[0])
    max_samples = 1000
    samples_per_query = 50
    # Use ThreadPoolExecutor to run generate_from_chat concurrently
    with ThreadPoolExecutor(max_workers=10) as executor:
        futures = [executor.submit(generate_from_chat, base_url, user_id, memory_prompt_id, max_samples, samples_per_query) for user_id in user_ids]
        
        for future in as_completed(futures):
            try:
                response = future.result()
                if response:
                    memories = response.json()
                    total_tokens = count_memory_tokens(memories)
                    print(f"Total tokens in generated memories: {total_tokens}")
                    
                    for memory in memories:
                        log_file_path = os.path.join(cwd, 'logs', f'{memory_prompt_id}-{memory["user_id"]}.txt')
                        if not os.path.exists(os.path.join(cwd, 'logs')):
                            os.makedirs(os.path.join (cwd, 'logs'), exist_ok=True)
                        with open(log_file_path, 'a') as log_file:
                            log_file.write(f'{memory["id"]},{memory["content"]}, {memory["grouping"]}, {memory["emoji"]}\n')

            except Exception as exc:
                print(f'Generated an exception: {exc}')
