import { DashboardLayout } from './components/layout/DashboardLayout';

function App() {
  return (
    <DashboardLayout>
      <div className="grid grid-cols-1 gap-4 mb-4">
        <div className="flex items-center justify-center h-24 rounded-sm bg-gray-50 dark:bg-gray-800">
          <p className="text-2xl text-gray-400 dark:text-gray-500">
            Welcome to Administration
          </p>
        </div>
      </div>
      <div className="grid grid-cols-2 gap-4 mb-4">
        <div className="flex items-center justify-center rounded-sm bg-gray-50 h-28 dark:bg-gray-800">
          <p className="text-2xl text-gray-400 dark:text-gray-500">
            Card 1
          </p>
        </div>
        <div className="flex items-center justify-center rounded-sm bg-gray-50 h-28 dark:bg-gray-800">
          <p className="text-2xl text-gray-400 dark:text-gray-500">
            Card 2
          </p>
        </div>
      </div>
    </DashboardLayout>
  );
}

export default App;
