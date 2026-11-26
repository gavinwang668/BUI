<script setup>
import { ref } from 'vue'
import { Button } from '@blender-ui/vue'

const count = ref(0)
const name = ref('')
</script>

<template>
  <main class="flex flex-col gap-3 p-6">
    <h1 class="text-xl font-bold">{{title}}</h1>
    <div class="flex items-center gap-3">
      <span>Clicks: {{ count }}</span>
      <Button @click="count++">Add one</Button>
    </div>
    <input placeholder="Your name" v-model="name" />
    <p v-if="name" class="text-[#9ab8e8]">Hello, {{ name }}!</p>
  </main>
</template>
