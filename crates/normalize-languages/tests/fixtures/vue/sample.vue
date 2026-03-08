<template>
  <div class="user-profile">
    <h1>{{ user.name }}</h1>
    <p v-if="user.bio">{{ user.bio }}</p>
    <p v-else>No bio provided.</p>

    <ul>
      <li v-for="post in posts" :key="post.id">
        <a :href="`/posts/${post.id}`">{{ post.title }}</a>
      </li>
    </ul>

    <button @click="loadMore" :disabled="loading">
      {{ loading ? 'Loading...' : 'Load more' }}
    </button>

    <slot name="footer" />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { useRoute } from 'vue-router';
import type { User, Post } from '@/types';
import { fetchUser, fetchPosts } from '@/api';

const props = defineProps<{
  userId: string;
  showPosts?: boolean;
}>();

const emit = defineEmits<{
  (e: 'loaded', user: User): void;
}>();

const route = useRoute();
const user = ref<User | null>(null);
const posts = ref<Post[]>([]);
const loading = ref(false);
const page = ref(1);

const hasMore = computed(() => posts.value.length % 10 === 0);

async function loadUser() {
  user.value = await fetchUser(props.userId);
  if (user.value) emit('loaded', user.value);
}

async function loadMore() {
  loading.value = true;
  const newPosts = await fetchPosts(props.userId, page.value++);
  posts.value.push(...newPosts);
  loading.value = false;
}

onMounted(async () => {
  await loadUser();
  if (props.showPosts) await loadMore();
});
</script>

<style scoped>
.user-profile {
  max-width: 800px;
  margin: 0 auto;
  padding: 1rem;
}

h1 {
  font-size: 2rem;
  margin-bottom: 0.5rem;
}

ul {
  list-style: none;
  padding: 0;
}

button {
  margin-top: 1rem;
  padding: 0.5rem 1rem;
  cursor: pointer;
}
</style>
