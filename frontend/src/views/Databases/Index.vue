<template>
  <div class="databases-view">
    <header>
      <h1>Databases Management</h1>
      <button @click="showCreateModal = true">Create Database</button>
    </header>

    <section class="lists-container">
      <div class="databases-list">
        <h2>Databases</h2>
        <ul>
          <li v-for="db in databases" :key="db.name">
            {{ db.name }} (Owner: {{ db.owner }})
            <button @click="triggerDump(db.name)">Download Dump</button>
          </li>
        </ul>
      </div>

      <div class="users-list">
        <h2>Users</h2>
        <ul>
          <li v-for="user in users" :key="user.name">
            {{ user.name }}
          </li>
        </ul>
      </div>
    </section>

    <section class="privileges-matrix">
      <h2>Privilege Assignment</h2>
      <table>
        <thead>
          <tr>
            <th>Database \ User</th>
            <th v-for="user in users" :key="user.name">{{ user.name }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="db in databases" :key="db.name">
            <td>{{ db.name }}</td>
            <td v-for="user in users" :key="user.name">
              <select v-model="privileges[db.name + '-' + user.name]" @change="assignPrivilege(db.name, user.name, $event.target.value)">
                <option value="">None</option>
                <option value="SELECT">SELECT</option>
                <option value="INSERT">INSERT</option>
                <option value="ALL">ALL PRIVILEGES</option>
              </select>
            </td>
          </tr>
        </tbody>
      </table>
    </section>

    <!-- Create Database Modal -->
    <div v-if="showCreateModal" class="modal">
      <div class="modal-content">
        <h2>Create New Database</h2>
        <form @submit.prevent="createDatabase">
          <div>
            <label>Database Name:</label>
            <input type="text" v-model="newDb.name" @input="validateIdentifier(newDb.name, 'nameError')" />
            <span class="error" v-if="errors.nameError">{{ errors.nameError }}</span>
          </div>
          <div>
            <label>Owner (Username):</label>
            <input type="text" v-model="newDb.owner" @input="validateIdentifier(newDb.owner, 'ownerError')" />
            <span class="error" v-if="errors.ownerError">{{ errors.ownerError }}</span>
          </div>
          <div>
            <button type="submit" :disabled="!isFormValid">Create</button>
            <button type="button" @click="showCreateModal = false">Cancel</button>
          </div>
        </form>
      </div>
    </div>
  </div>
</template>

<script>
export default {
  name: 'DatabasesView',
  data() {
    return {
      showCreateModal: false,
      databases: [
        { name: 'app_db', owner: 'app_user' },
        { name: 'analytics_db', owner: 'analytics_user' }
      ],
      users: [
        { name: 'app_user' },
        { name: 'analytics_user' },
        { name: 'readonly_user' }
      ],
      privileges: {},
      newDb: {
        name: '',
        owner: ''
      },
      errors: {
        nameError: '',
        ownerError: ''
      }
    }
  },
  computed: {
    isFormValid() {
      return this.newDb.name.length > 0 &&
             this.newDb.owner.length > 0 &&
             !this.errors.nameError &&
             !this.errors.ownerError;
    }
  },
  methods: {
    validateIdentifier(id, errorKey) {
      this.errors[errorKey] = '';
      if (!id) {
         this.errors[errorKey] = 'Identifier is required';
         return;
      }
      if (id.length > 63) {
         this.errors[errorKey] = 'Identifier must be 1-63 characters';
         return;
      }
      if (/^\d/.test(id)) {
         this.errors[errorKey] = 'Identifier cannot start with a digit';
         return;
      }
      if (!/^[a-zA-Z0-9_]+$/.test(id)) {
         this.errors[errorKey] = 'Identifier can only contain alphanumeric characters and underscores';
         return;
      }
    },
    createDatabase() {
      if (this.isFormValid) {
        // In a real app, this would be an API call
        this.databases.push({ ...this.newDb });
        if (!this.users.find(u => u.name === this.newDb.owner)) {
          this.users.push({ name: this.newDb.owner });
        }
        this.showCreateModal = false;
        this.newDb.name = '';
        this.newDb.owner = '';
      }
    },
    triggerDump(dbName) {
      // Dummy dump download trigger
      console.log(`Triggering dump for ${dbName}`);
      alert(`Dump download initiated for ${dbName}`);
    },
    assignPrivilege(dbName, userName, privilege) {
      // Dummy privilege assignment
      console.log(`Assigning ${privilege} to ${userName} on ${dbName}`);
    }
  }
}
</script>

<style scoped>
.databases-view {
  padding: 20px;
}
.modal {
  position: fixed;
  top: 0; left: 0; right: 0; bottom: 0;
  background: rgba(0,0,0,0.5);
  display: flex;
  align-items: center;
  justify-content: center;
}
.modal-content {
  background: white;
  padding: 20px;
  border-radius: 4px;
}
.error {
  color: red;
  font-size: 0.8em;
  display: block;
}
table {
  width: 100%;
  border-collapse: collapse;
}
th, td {
  border: 1px solid #ccc;
  padding: 8px;
  text-align: left;
}
</style>
