import requests
import uuid
import os

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

def generate_from_chat(base_url, user_id, memory_prompt_id=None, n_samples=None):
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
    
    if n_samples is not None:
        payload["n_samples"] = n_samples

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

# Example usage
# TODO
'''
- run the new prompt on 10 random users + sully & i
- log it all
- read it and determine if memory useful or not
- use chats & memories to create few-shot examples
- iterate
'''
if __name__ == '__main__':
    base_url = "http://localhost:8000/memory"
                # sully                             # minjune                       # some russian guy
    user_ids = ["user_01J0KQQ0XVK8K2FWV6AGTNJ6EG", "user_01HY5EW9Z5XVE34GZXKH4NC2Y1", "user_01HZEP4TFR49AG913DPQJ6MASW",] 
    memory_prompt_id = "e9ce3939-3143-4552-8c71-e7e741b65493"  # Generate a random UUID
    n_samples = 100

    for p in os.listdir('prompts'):
        pf = os.path.join('prompts', p)
        with open(pf, 'r') as f:
            prompt = f.read()
            #add_memory(base_url, prompt)

    for user_id in user_ids:
        generate_from_chat(base_url, user_id, memory_prompt_id, n_samples)
